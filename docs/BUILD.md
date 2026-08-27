# Building Formiga

Rust 1.97.1 is pinned by `rust-toolchain.toml`.

## Development checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p formiga-tools -- simulate 181
cargo run -p formiga-desktop
```

The desktop binary supports macOS 14+ and Windows 10/11 x64. Core simulation, art, habitat,
persistence, drag-state, and occlusion-region tests are platform-independent. Native overlay and
proxy behavior still requires the manual OS matrix in `TEST_MATRIX.md`.

## Downloads for nontechnical users

Tagged builds publish four ready-to-run files on GitHub Releases. Recommend the macOS DMG and
Windows MSI; the ZIP files are portable alternatives. Opening Formiga for the first time creates a
colony and opens Settings automatically. No Rust installation, command line, or separate runtime is
needed.

The release tag is the package source of truth. A tag such as `v0.32.0` builds an application that
reports version `0.32.0`, places the same version in the macOS bundle and Windows MSI, and publishes
the exact updater-compatible names `Formiga-0.32.0-macOS-universal.dmg` and
`Formiga-0.32.0-windows-x64.msi`. Do not rename these two assets after publishing. Their companion
`.sha256` files are the fallback verification source when GitHub release metadata does not provide a
digest.

## macOS universal preview

Run `bash scripts/package-macos.sh` on macOS. It creates an arm64/x86_64 universal app, ad-hoc signs
it when no identity is supplied, and writes a drag-to-Applications DMG, ZIP, and SHA-256 checksums
to `dist/`.

Set `FORMIGA_CODESIGN_IDENTITY` to a Developer ID Application identity for distribution signing.
Notarization requires release-owner Apple credentials and is intentionally not embedded in the
repository.

Unsigned/ad-hoc portfolio preview: after downloading, Control-click the app and choose **Open**. Do
not advise users to disable Gatekeeper globally.

## Windows preview

Run `./scripts/package-windows.ps1` in PowerShell. With WiX 4 installed it creates a per-user MSI
with Desktop and Start-menu shortcuts, a portable ZIP, and SHA-256 checksum files. Use
`-SkipInstaller` for the ZIP only.

Unsigned portfolio preview: Windows SmartScreen may show **More info → Run anyway**. Authenticode
signing requires the release owner's certificate and remains a release-secret hook.

GitHub Actions checks both operating systems and packages unsigned preview artifacts. Tags matching
`v*` feed the release workflow. The current workflow marks portfolio builds as prereleases; the app
intentionally checks those releases as well as stable releases.
