use anyhow::{Context, Result, anyhow, bail};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use time::OffsetDateTime;

const RELEASES_URL: &str =
    "https://api.github.com/repos/Von-Van/Formiga-Desktop/releases?per_page=10";
const MAX_INSTALLER_BYTES: u64 = 250 * 1024 * 1024;
const CHECK_INTERVAL_SECONDS: i64 = 24 * 60 * 60;
/// How long after a check that did not get an answer the next automatic one may start.
const RETRY_INTERVAL_SECONDS: i64 = 60 * 60;
/// What an installer Formiga downloaded is called, after the version in the middle of the name.
const INSTALLER_SUFFIXES: [&str; 2] = ["-macOS-universal.dmg", "-windows-x64.msi"];
/// How long a part-finished download sits untouched before it counts as abandoned.
const ABANDONED_DOWNLOAD_SECONDS: u64 = 24 * 60 * 60;

pub const APP_VERSION: &str = match option_env!("FORMIGA_BUILD_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateAsset {
    pub name: String,
    pub download_url: String,
    pub size: u64,
    pub sha256: Option<String>,
    pub checksum_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateRelease {
    pub version: String,
    pub notes: String,
    pub page_url: String,
    pub prerelease: bool,
    pub asset: UpdateAsset,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadedUpdate {
    pub release: UpdateRelease,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate {
        checked_at_unix: i64,
    },
    Available(UpdateRelease),
    Downloading(UpdateRelease),
    Ready(DownloadedUpdate),
    Failed(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct UpdatePreferences {
    automatic_checks: bool,
    last_checked_unix: Option<i64>,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            automatic_checks: true,
            last_checked_unix: None,
        }
    }
}

pub struct UpdateController {
    preferences_path: PathBuf,
    download_dir: PathBuf,
    preferences: UpdatePreferences,
    status: UpdateStatus,
    /// When the last check started, answered or not; only an answer counts as checked, so this
    /// spaces out the retries while the network is away.
    last_attempt_unix: Option<i64>,
    /// Whether the check under way opens the About page when it finds a release: yes for one the
    /// owner asked for or one made as Formiga starts, no for the daily one while it runs.
    reveal_found: bool,
}

impl UpdateController {
    pub fn load(data_dir: &Path) -> Self {
        let preferences_path = data_dir.join("updates.json");
        let preferences = fs::read(&preferences_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let controller = Self {
            preferences_path,
            download_dir: data_dir.join("updates"),
            preferences,
            status: UpdateStatus::Idle,
            last_attempt_unix: None,
            reveal_found: true,
        };
        controller.clear_used_downloads();
        controller
    }

    /// Clear out the installers that have done their job, so the updates folder does not end up
    /// holding a copy of every version the colony has ever run. Done once, as Formiga starts.
    fn clear_used_downloads(&self) {
        if running_from_a_mounted_image() {
            return;
        }
        let Ok(current) = parse_version(APP_VERSION) else {
            return;
        };
        let cleared = clear_used_installers(&self.download_dir, &current, SystemTime::now());
        if !cleared.is_empty() {
            tracing::info!(count = cleared.len(), "cleared installers already used");
        }
    }

    pub fn automatic_checks(&self) -> bool {
        self.preferences.automatic_checks
    }

    pub fn set_automatic_checks(&mut self, enabled: bool) -> Result<()> {
        self.preferences.automatic_checks = enabled;
        self.save_preferences()
    }

    /// Whether an automatic check is due: a day after the last one that got an answer, and an
    /// hour after the last one that did not. A release already found waits for the owner instead.
    pub fn should_check_automatically(&self, now: OffsetDateTime) -> bool {
        let now = now.unix_timestamp();
        self.preferences.automatic_checks
            && !matches!(
                self.status,
                UpdateStatus::Checking
                    | UpdateStatus::Available(_)
                    | UpdateStatus::Downloading(_)
                    | UpdateStatus::Ready(_)
            )
            && self
                .preferences
                .last_checked_unix
                .is_none_or(|last| now.saturating_sub(last) >= CHECK_INTERVAL_SECONDS)
            && self
                .last_attempt_unix
                .is_none_or(|last| now.saturating_sub(last) >= RETRY_INTERVAL_SECONDS)
    }

    pub fn status(&self) -> &UpdateStatus {
        &self.status
    }

    /// Starts a check unless one or a download is under way. `reveal` says whether a release it
    /// finds should open the About page.
    pub fn begin_check(&mut self, reveal: bool, now: OffsetDateTime) -> bool {
        if matches!(
            self.status,
            UpdateStatus::Checking | UpdateStatus::Downloading(_)
        ) {
            return false;
        }
        self.status = UpdateStatus::Checking;
        self.last_attempt_unix = Some(now.unix_timestamp());
        self.reveal_found = reveal;
        true
    }

    /// Takes in a check's answer, returning whether it should open the About page. Only an answer
    /// counts as checked: one that failed is tried again within the hour rather than tomorrow.
    pub fn finish_check(
        &mut self,
        result: Result<Option<UpdateRelease>, String>,
        now: OffsetDateTime,
    ) -> bool {
        let now = now.unix_timestamp();
        if result.is_ok() {
            self.preferences.last_checked_unix = Some(now);
            if let Err(error) = self.save_preferences() {
                tracing::warn!(%error, "could not persist update-check time");
            }
        }
        match result {
            Ok(Some(release)) => {
                self.status = UpdateStatus::Available(release);
                self.reveal_found
            }
            Ok(None) => {
                self.status = UpdateStatus::UpToDate {
                    checked_at_unix: now,
                };
                false
            }
            Err(error) => {
                self.status = UpdateStatus::Failed(error);
                false
            }
        }
    }

    pub fn begin_download(&mut self) -> Option<(UpdateRelease, PathBuf)> {
        let release = match &self.status {
            UpdateStatus::Available(release) => release.clone(),
            _ => return None,
        };
        self.status = UpdateStatus::Downloading(release.clone());
        Some((release, self.download_dir.clone()))
    }

    pub fn finish_download(&mut self, result: Result<DownloadedUpdate, String>) {
        self.status = match result {
            Ok(downloaded) => UpdateStatus::Ready(downloaded),
            Err(error) => UpdateStatus::Failed(error),
        };
    }

    pub fn ready_update(&self) -> Option<&DownloadedUpdate> {
        match &self.status {
            UpdateStatus::Ready(downloaded) => Some(downloaded),
            _ => None,
        }
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = UpdateStatus::Failed(error.into());
    }

    fn save_preferences(&self) -> Result<()> {
        if let Some(parent) = self.preferences_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.preferences_path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(&self.preferences)?;
        let mut file = File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        // A rename replaces the old file in one step on macOS and Windows alike, so a crash
        // never leaves the choice to stop checking missing, and so turned back on.
        fs::rename(temporary, &self.preferences_path)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    assets: Vec<ApiAsset>,
}

#[derive(Debug, Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

pub fn check_github() -> Result<Option<UpdateRelease>> {
    let agent = http_agent(Duration::from_secs(15));
    let body = agent
        .get(RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", concat!("Formiga/", env!("CARGO_PKG_VERSION")))
        .call()
        .context("contact GitHub Releases")?
        .body_mut()
        .read_to_string()
        .context("read GitHub release metadata")?;
    let releases: Vec<ApiRelease> =
        serde_json::from_str(&body).context("decode GitHub release metadata")?;
    select_update(releases)
}

fn select_update(releases: Vec<ApiRelease>) -> Result<Option<UpdateRelease>> {
    let current = parse_version(APP_VERSION).context("parse Formiga's installed version")?;
    let mut candidates = Vec::new();
    for release in releases.into_iter().filter(|release| !release.draft) {
        let Ok(version) = parse_version(&release.tag_name) else {
            continue;
        };
        if version <= current {
            continue;
        }
        let Some(asset) = select_platform_asset(&release.assets, &version.to_string()) else {
            continue;
        };
        let checksum_name = format!("{}.sha256", asset.name);
        let checksum_url = release
            .assets
            .iter()
            .find(|candidate| candidate.name == checksum_name)
            .map(|candidate| candidate.browser_download_url.clone());
        let sha256 = asset
            .digest
            .as_deref()
            .and_then(|digest| digest.strip_prefix("sha256:"))
            .map(str::to_owned);
        candidates.push((
            version.clone(),
            UpdateRelease {
                version: version.to_string(),
                notes: release
                    .body
                    .unwrap_or_default()
                    .chars()
                    .take(4_000)
                    .collect(),
                page_url: release.html_url,
                prerelease: release.prerelease,
                asset: UpdateAsset {
                    name: asset.name.clone(),
                    download_url: asset.browser_download_url.clone(),
                    size: asset.size,
                    sha256,
                    checksum_url,
                },
            },
        ));
    }
    Ok(candidates
        .into_iter()
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, release)| release))
}

fn select_platform_asset<'a>(assets: &'a [ApiAsset], version: &str) -> Option<&'a ApiAsset> {
    #[cfg(target_os = "macos")]
    let expected = format!("Formiga-{version}-macOS-universal.dmg");
    #[cfg(target_os = "windows")]
    let expected = format!("Formiga-{version}-windows-x64.msi");
    assets.iter().find(|asset| asset.name == expected)
}

fn parse_version(value: &str) -> Result<Version> {
    Version::parse(value.trim().trim_start_matches(['v', 'V'])).map_err(Into::into)
}

pub fn download_update(release: UpdateRelease, directory: PathBuf) -> Result<DownloadedUpdate> {
    if release.asset.size == 0 || release.asset.size > MAX_INSTALLER_BYTES {
        bail!("update installer has an unexpected size");
    }
    let file_name = Path::new(&release.asset.name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name == release.asset.name)
        .context("update asset has an unsafe file name")?;
    fs::create_dir_all(&directory)?;
    let final_path = directory.join(file_name);
    let temporary = directory.join(format!("{file_name}.part"));
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }

    let expected = match &release.asset.sha256 {
        Some(digest) => validate_digest(digest)?.to_owned(),
        None => {
            let url = release
                .asset
                .checksum_url
                .as_deref()
                .context("release does not provide a SHA-256 checksum")?;
            fetch_checksum(url)?
        }
    };

    let agent = http_agent(Duration::from_secs(5 * 60));
    let mut response = agent
        .get(&release.asset.download_url)
        .header("User-Agent", concat!("Formiga/", env!("CARGO_PKG_VERSION")))
        .call()
        .context("download update installer")?;
    let mut reader = response.body_mut().as_reader();
    receive_installer(&mut reader, &temporary, release.asset.size, &expected)?;
    fs::rename(&temporary, &final_path)?;
    Ok(DownloadedUpdate {
        release,
        path: final_path,
    })
}

/// Writes an installer to `temporary` as it arrives, checking its size and digest. One that fails
/// part way is removed rather than left for the day it takes to count as abandoned: an installer
/// can be a few hundred megabytes.
fn receive_installer(
    reader: &mut impl Read,
    temporary: &Path,
    size: u64,
    expected: &str,
) -> Result<()> {
    let result = write_installer(reader, temporary, size, expected);
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn write_installer(
    reader: &mut impl Read,
    temporary: &Path,
    size: u64,
    expected: &str,
) -> Result<()> {
    let mut file = File::create(temporary)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > MAX_INSTALLER_BYTES || total > size.saturating_add(1024) {
            bail!("download exceeded the advertised installer size");
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])?;
    }
    file.sync_all()?;
    if total != size {
        bail!(
            "downloaded installer size did not match GitHub metadata (expected {size}, received {total})"
        );
    }
    let actual = format!("{:x}", hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("downloaded installer failed SHA-256 verification");
    }
    Ok(())
}

/// The version an installer would install, for the names Formiga's own downloads carry and no
/// others. Anything else in the folder belongs to whoever put it there.
fn installer_version(name: &str) -> Option<Version> {
    let rest = name.strip_prefix("Formiga-")?;
    let version = INSTALLER_SUFFIXES
        .iter()
        .find_map(|suffix| rest.strip_suffix(suffix))?;
    Version::parse(version).ok()
}

/// Whether a part-finished download has sat unfinished long enough to count as abandoned.
fn abandoned_part(entry: &fs::DirEntry, name: &str, now: SystemTime) -> bool {
    let Some(installer) = name.strip_suffix(".part") else {
        return false;
    };
    installer_version(installer).is_some()
        && entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age.as_secs() >= ABANDONED_DOWNLOAD_SECONDS)
}

/// Remove the installers in `directory` that are finished with: any that would install the running
/// version or an older one, and any part-finished download left for a day or more. An installer for
/// a later version stays, since it may not have been installed yet, and so does every file that is
/// not one of Formiga's own downloads.
///
/// This is housekeeping rather than a step that has to succeed: whatever cannot be removed is left
/// where it is. Returns the names it cleared.
fn clear_used_installers(directory: &Path, current: &Version, now: SystemTime) -> Vec<String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut cleared = Vec::new();
    for entry in entries.flatten() {
        // A symlink is somebody else's business, whatever it is called.
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let finished_with = match installer_version(name) {
            Some(version) => version <= *current,
            None => abandoned_part(&entry, name, now),
        };
        if !finished_with {
            continue;
        }
        match fs::remove_file(entry.path()) {
            Ok(()) => cleared.push(name.to_owned()),
            Err(error) => tracing::debug!(%error, name, "could not clear a used installer"),
        }
    }
    cleared
}

/// Whether Formiga is running from a mounted disk image or another volume it does not own, in
/// which case the installer it came from is still holding the app that is running and is left be.
#[cfg(target_os = "macos")]
fn running_from_a_mounted_image() -> bool {
    std::env::current_exe().is_ok_and(|path| path.starts_with("/Volumes/"))
}

#[cfg(not(target_os = "macos"))]
fn running_from_a_mounted_image() -> bool {
    false
}

fn fetch_checksum(url: &str) -> Result<String> {
    let agent = http_agent(Duration::from_secs(15));
    let body = agent
        .get(url)
        .header("User-Agent", concat!("Formiga/", env!("CARGO_PKG_VERSION")))
        .call()
        .context("download update checksum")?
        .body_mut()
        .read_to_string()
        .context("read update checksum")?;
    let digest = body
        .split_whitespace()
        .next()
        .context("checksum file was empty")?;
    Ok(validate_digest(digest)?.to_owned())
}

fn validate_digest(digest: &str) -> Result<&str> {
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(anyhow!("release contains an invalid SHA-256 checksum"));
    }
    Ok(digest)
}

/// Every request the updater makes, and every redirect it follows, stays on HTTPS.
fn http_agent(timeout: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .https_only(true)
        .build();
    config.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str, digest: Option<&str>) -> ApiAsset {
        ApiAsset {
            name: name.into(),
            browser_download_url: format!("https://example.invalid/{name}"),
            size: 1024,
            digest: digest.map(str::to_owned),
        }
    }

    #[test]
    fn rejects_invalid_digests_and_unsafe_file_names() {
        assert!(validate_digest("abc").is_err());
        assert!(validate_digest(&"a".repeat(64)).is_ok());
        assert_ne!(
            Path::new("../Formiga.msi")
                .file_name()
                .and_then(|name| name.to_str()),
            Some("../Formiga.msi")
        );
    }

    #[test]
    fn the_updater_only_speaks_https() {
        assert!(http_agent(Duration::from_secs(1)).config().https_only());
    }

    #[test]
    fn release_selection_uses_highest_newer_compatible_asset() {
        let current = parse_version(APP_VERSION).unwrap();
        let next = Version::new(current.major, current.minor + 1, 0);
        let later = Version::new(current.major, current.minor + 2, 0);
        #[cfg(target_os = "macos")]
        let package_name = |version: &Version| format!("Formiga-{version}-macOS-universal.dmg");
        #[cfg(target_os = "windows")]
        let package_name = |version: &Version| format!("Formiga-{version}-windows-x64.msi");
        let digest = format!("sha256:{}", "b".repeat(64));
        let releases = vec![
            ApiRelease {
                tag_name: format!("v{next}"),
                html_url: "https://example.invalid/next".into(),
                body: Some("next".into()),
                draft: false,
                prerelease: true,
                assets: vec![asset(&package_name(&next), Some(&digest))],
            },
            ApiRelease {
                tag_name: format!("v{later}"),
                html_url: "https://example.invalid/later".into(),
                body: Some("later".into()),
                draft: false,
                prerelease: true,
                assets: vec![asset(&package_name(&later), Some(&digest))],
            },
        ];
        let selected = select_update(releases).unwrap().unwrap();
        assert_eq!(selected.version, later.to_string());
        assert_eq!(selected.sha256_for_test(), "b".repeat(64));
    }

    impl UpdateRelease {
        fn sha256_for_test(&self) -> String {
            self.asset.sha256.clone().unwrap()
        }
    }

    /// An empty folder of its own, named for the test using it.
    fn scratch(label: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("formiga-updater-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    /// Put a file's last-changed time back, as if it had been sitting there.
    fn age(path: &Path, by: Duration) {
        let when = SystemTime::now() - by;
        let file = File::options().write(true).open(path).unwrap();
        file.set_times(fs::FileTimes::new().set_accessed(when).set_modified(when))
            .unwrap();
    }

    #[test]
    fn only_formigas_own_installer_names_are_recognized() {
        assert_eq!(
            installer_version("Formiga-0.59.2-macOS-universal.dmg"),
            Some(Version::new(0, 59, 2))
        );
        assert_eq!(
            installer_version("Formiga-1.0.0-rc.1-windows-x64.msi"),
            Version::parse("1.0.0-rc.1").ok()
        );
        for name in [
            "Formiga--macOS-universal.dmg",
            "Formiga-0.59.2-macOS-universal.zip",
            "../Formiga-0.59.2-macOS-universal.dmg",
            "holiday.dmg",
        ] {
            assert!(installer_version(name).is_none(), "{name} is not ours");
        }
    }

    #[test]
    fn installers_already_used_are_cleared_and_later_ones_kept() {
        let directory = scratch("used");
        let kept = [
            "Formiga-0.60.1-macOS-universal.dmg",
            "Formiga-0.58.9-macOS-universal.zip",
            "colony-backup.dmg",
            "notes.txt",
        ];
        let used = [
            "Formiga-0.58.9-macOS-universal.dmg",
            "Formiga-0.59.2-windows-x64.msi",
            "Formiga-0.60.0-macOS-universal.dmg",
        ];
        for name in kept.iter().chain(used.iter()) {
            fs::write(directory.join(name), b"installer").unwrap();
        }

        let mut cleared =
            clear_used_installers(&directory, &Version::new(0, 60, 0), SystemTime::now());
        cleared.sort();

        assert_eq!(cleared, used);
        for name in kept {
            assert!(
                directory.join(name).exists(),
                "{name} should have been left"
            );
        }
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn an_abandoned_part_download_is_cleared_and_one_still_going_is_kept() {
        let directory = scratch("part");
        let abandoned = "Formiga-0.60.1-macOS-universal.dmg.part";
        let going = "Formiga-0.61.0-macOS-universal.dmg.part";
        let stranger = "somebody-elses.part";
        for name in [abandoned, going, stranger] {
            fs::write(directory.join(name), b"half a download").unwrap();
        }
        age(
            &directory.join(abandoned),
            Duration::from_secs(2 * 24 * 60 * 60),
        );

        let cleared = clear_used_installers(&directory, &Version::new(0, 60, 0), SystemTime::now());

        assert_eq!(cleared, vec![abandoned]);
        assert!(directory.join(going).exists());
        assert!(directory.join(stranger).exists());
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn automatic_check_is_throttled_for_twenty_four_hours() {
        let directory = scratch("daily");
        let mut controller = UpdateController::load(&directory);
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::days(10);
        assert!(controller.should_check_automatically(now));
        assert!(controller.begin_check(false, now));
        assert!(!controller.finish_check(Ok(None), now));
        assert!(!controller.should_check_automatically(now + time::Duration::hours(23)));
        assert!(controller.should_check_automatically(now + time::Duration::hours(24)));
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn a_check_that_gets_no_answer_is_tried_again_within_the_hour() {
        let directory = scratch("retry");
        let mut controller = UpdateController::load(&directory);
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::days(10);
        assert!(controller.begin_check(true, now));
        controller.finish_check(Err("offline".into()), now);

        assert_eq!(controller.preferences.last_checked_unix, None);
        assert!(!directory.join("updates.json").exists());
        assert!(!controller.should_check_automatically(now + time::Duration::minutes(59)));
        assert!(controller.should_check_automatically(now + time::Duration::hours(1)));
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn a_release_found_in_the_background_waits_in_the_tray() {
        let directory = scratch("reveal");
        let mut controller = UpdateController::load(&directory);
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::days(10);
        let release = || {
            Some(UpdateRelease {
                version: "99.0.0".into(),
                notes: String::new(),
                page_url: "https://example.invalid/release".into(),
                prerelease: false,
                asset: UpdateAsset {
                    name: "Formiga-99.0.0-macOS-universal.dmg".into(),
                    download_url: "https://example.invalid/installer".into(),
                    size: 1024,
                    sha256: Some("b".repeat(64)),
                    checksum_url: None,
                },
            })
        };

        assert!(controller.begin_check(false, now));
        assert!(!controller.finish_check(Ok(release()), now));
        assert!(matches!(controller.status(), UpdateStatus::Available(_)));
        // Found, it waits for the owner rather than being looked for again.
        assert!(!controller.should_check_automatically(now + time::Duration::days(2)));

        assert!(controller.begin_check(true, now));
        assert!(controller.finish_check(Ok(release()), now));
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn the_choice_to_stop_checking_is_kept_when_written_again() {
        let directory = scratch("preferences");
        let mut controller = UpdateController::load(&directory);
        controller.set_automatic_checks(false).unwrap();
        controller.set_automatic_checks(false).unwrap();

        assert!(!UpdateController::load(&directory).automatic_checks());
        assert!(!directory.join("updates.json.tmp").exists());
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn an_installer_that_fails_part_way_leaves_nothing_behind() {
        let directory = scratch("receive");
        let temporary = directory.join("Formiga-9.9.9-macOS-universal.dmg.part");
        let body = b"most of an installer";
        let digest = format!("{:x}", Sha256::digest(body));

        let short = receive_installer(&mut &body[..], &temporary, 1_000, &digest);
        assert!(short.is_err());
        assert!(!temporary.exists());

        let wrong = receive_installer(
            &mut &body[..],
            &temporary,
            body.len() as u64,
            &"0".repeat(64),
        );
        assert!(wrong.is_err());
        assert!(!temporary.exists());

        receive_installer(&mut &body[..], &temporary, body.len() as u64, &digest).unwrap();
        assert_eq!(fs::read(&temporary).unwrap(), body);
        fs::remove_dir_all(&directory).unwrap();
    }
}
