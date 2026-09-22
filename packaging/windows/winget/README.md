# winget manifest for Formiga

This directory holds a **template** winget manifest set for package identifier
`VonVan.Formiga` (manifest schema 1.6.0) and a script that fills it in for a specific released
version. Nothing here is submitted automatically - it only prepares the files a maintainer pastes
into a `microsoft/winget-pkgs` pull request.

## What's here

- `VonVan.Formiga.yaml.template` - the version manifest.
- `VonVan.Formiga.installer.yaml.template` - the installer manifest.
- `VonVan.Formiga.locale.en-US.yaml.template` - the default locale (metadata) manifest.
- `../../../scripts/winget-manifest.sh <version>` - reads the already-published GitHub release
  for `<version>`, verifies the Windows MSI's checksum, and writes filled-in copies of the three
  files above to `out/<version>/` (git-ignored - it's generated, not checked in).

## Facts a reviewer or installer should have upfront

- **The installer is unsigned.** Formiga ships without an Authenticode certificate (no Apple/
  Microsoft signing credentials yet), so Windows SmartScreen will show an "unrecognized app"
  warning on first run. The installer manifest does not (and cannot honestly) claim otherwise.
- **License:** MIT, linked straight to the `LICENSE` file in the repository.
- **Privacy:** linked to `docs/PRIVACY.md` on GitHub, which documents that Formiga has no
  account system, analytics, or ad SDK, and that its only network feature is an optional daily
  update check.
- **Release notes:** linked to the GitHub release tag itself, so winget's "what's new" points at
  the real changelog entry.
- **Scope is per-user** (installs under `%LocalAppData%\Formiga`, no admin rights expected) and
  **there is no `ProductCode`** in the installer manifest - see the comment block at the top of
  `VonVan.Formiga.installer.yaml.template` for why (short version: WiX 4 does not pin one in
  `packaging/windows/Formiga.wxs`, so it is not stable enough to read out of source, and
  extracting it from a built MSI needs tooling beyond this script's curl/gh/shasum/sed budget).

## Generating a submission for a released version

1. Make sure the release is actually published with its Windows assets, e.g.:
   ```sh
   gh release view v0.59.5 --json assets
   ```
2. Generate the manifest:
   ```sh
   scripts/winget-manifest.sh 0.59.5
   ```
   This downloads the released MSI to verify its SHA-256 against the published `.sha256` file
   (not just trusting the small checksum file blindly), then writes
   `packaging/windows/winget/out/0.59.5/VonVan.Formiga*.yaml`.
3. Validate the manifest shape (requires the `winget` client, i.e. run this on Windows):
   ```powershell
   winget validate --manifest packaging\windows\winget\out\0.59.5
   ```
4. Test an actual install from the manifest, ideally in a disposable VM since it downloads and
   runs the real installer:
   ```powershell
   winget install --manifest packaging\windows\winget\out\0.59.5
   ```
   Confirm: no elevation prompt (scope is per-user), SmartScreen's warning appears and "Run
   anyway" completes the install, and Start Menu/Desktop shortcuts land as expected.
5. Fork `microsoft/winget-pkgs`, copy the three generated files into
   `manifests/v/VonVan/Formiga/0.59.5/` in that fork (that path - lowercase first letter of the
   publisher, then `Publisher/Package/Version` - is winget-pkgs' required layout), commit, and
   open a pull request. `wingetcreate submit` can do the fork/branch/PR steps for you if
   preferred; see <https://github.com/microsoft/winget-create>.
6. Repeat steps 2-5 for every future release the owner wants published to winget - each version
   is its own manifest folder and its own PR.

## What could block acceptance for an unsigned prerelease

- **Every Formiga release today is an unsigned prerelease** (there is no Authenticode
  certificate yet, so `.github/workflows/release.yml` publishes every tag as a GitHub
  prerelease). `microsoft/winget-pkgs`'s moderators generally expect submissions to be a
  package's real public release, not a pre-release/beta channel; it is worth saying so plainly
  in the PR description rather than letting a reviewer discover it, and it would be reasonable
  for a moderator to ask that `winget-pkgs` track only tagged stable releases once Formiga has
  any.
- **Defender/SmartScreen reputation.** winget-pkgs' automated validation pipeline runs the
  installer through a Defender scan and checks binary reputation. A brand-new, unsigned,
  low-download-count installer from a repository with no stars yet is exactly the profile that
  trips "unknown publisher" heuristics, which typically routes the PR to manual moderator review
  instead of auto-merging - expect a delay, not necessarily a rejection.
- **No `ProductCode`.** Most accepted MSI manifests in the repository do carry one. A moderator
  may ask for it; if so, the easiest path is running `wingetcreate update` (it downloads the MSI
  and extracts `ProductCode` itself) rather than extending this shell script with an MSI parser.
- **Silent-install switches are assumed, not proven.** `InstallModes` lists `silent` and
  `silentWithProgress` because this is a plain WiX-built MSI (standard `msiexec` switches should
  apply), but that has not been exercised against winget's own automated silent-install test in
  this environment - step 4 above is where that gets caught before submission, not after.
- **One package, one identity.** If the owner ever changes the installer's `UpgradeCode`,
  `Scope`, or publisher name in `packaging/windows/Formiga.wxs`, future winget submissions need
  to reflect that too - this manifest set encodes today's `Formiga.wxs` as read on 2026-09-18,
  not a promise that it will never change.
