use anyhow::{Context, Result, anyhow, bail};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use time::OffsetDateTime;

const RELEASES_URL: &str =
    "https://api.github.com/repos/Von-Van/Formiga-Desktop/releases?per_page=10";
const MAX_INSTALLER_BYTES: u64 = 250 * 1024 * 1024;
const CHECK_INTERVAL_SECONDS: i64 = 24 * 60 * 60;

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
}

impl UpdateController {
    pub fn load(data_dir: &Path) -> Self {
        let preferences_path = data_dir.join("updates.json");
        let preferences = fs::read(&preferences_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            preferences_path,
            download_dir: data_dir.join("updates"),
            preferences,
            status: UpdateStatus::Idle,
        }
    }

    pub fn automatic_checks(&self) -> bool {
        self.preferences.automatic_checks
    }

    pub fn set_automatic_checks(&mut self, enabled: bool) -> Result<()> {
        self.preferences.automatic_checks = enabled;
        self.save_preferences()
    }

    pub fn should_check_automatically(&self, now: OffsetDateTime) -> bool {
        self.preferences.automatic_checks
            && !matches!(
                self.status,
                UpdateStatus::Checking | UpdateStatus::Downloading(_)
            )
            && self.preferences.last_checked_unix.is_none_or(|last| {
                now.unix_timestamp().saturating_sub(last) >= CHECK_INTERVAL_SECONDS
            })
    }

    pub fn status(&self) -> &UpdateStatus {
        &self.status
    }

    pub fn begin_check(&mut self) -> bool {
        if matches!(
            self.status,
            UpdateStatus::Checking | UpdateStatus::Downloading(_)
        ) {
            return false;
        }
        self.status = UpdateStatus::Checking;
        true
    }

    pub fn finish_check(&mut self, result: Result<Option<UpdateRelease>, String>) -> bool {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        self.preferences.last_checked_unix = Some(now);
        if let Err(error) = self.save_preferences() {
            tracing::warn!(%error, "could not persist update-check time");
        }
        match result {
            Ok(Some(release)) => {
                self.status = UpdateStatus::Available(release);
                true
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
        if self.preferences_path.exists() {
            fs::remove_file(&self.preferences_path)?;
        }
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
    let mut file = File::create(&temporary)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
        if total > MAX_INSTALLER_BYTES || total > release.asset.size.saturating_add(1024) {
            bail!("download exceeded the advertised installer size");
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])?;
    }
    file.sync_all()?;
    if total != release.asset.size {
        bail!(
            "downloaded installer size did not match GitHub metadata (expected {}, received {total})",
            release.asset.size
        );
    }
    let actual = format!("{:x}", hasher.finalize());
    if !actual.eq_ignore_ascii_case(&expected) {
        bail!("downloaded installer failed SHA-256 verification");
    }
    if final_path.exists() {
        fs::remove_file(&final_path)?;
    }
    fs::rename(&temporary, &final_path)?;
    Ok(DownloadedUpdate {
        release,
        path: final_path,
    })
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

fn http_agent(timeout: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
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
            Path::new("../Formiga.msi").file_name().and_then(|name| name.to_str()),
            Some("../Formiga.msi")
        );
    }

    #[test]
    fn release_selection_uses_highest_newer_compatible_asset() {
        let current = parse_version(APP_VERSION).unwrap();
        let next = Version::new(current.major, current.minor + 1, 0);
        let later = Version::new(current.major, current.minor + 2, 0);
        #[cfg(target_os = "macos")]
        let package_name = |version: &Version| {
            format!("Formiga-{version}-macOS-universal.dmg")
        };
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

    #[test]
    fn automatic_check_is_throttled_for_twenty_four_hours() {
        let temporary = std::env::temp_dir().join(format!(
            "formiga-updater-test-{}",
            std::process::id()
        ));
        let mut controller = UpdateController::load(&temporary);
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::days(10);
        assert!(controller.should_check_automatically(now));
        controller.preferences.last_checked_unix = Some(now.unix_timestamp());
        assert!(!controller.should_check_automatically(now + time::Duration::hours(23)));
        assert!(controller.should_check_automatically(now + time::Duration::hours(24)));
    }
}
