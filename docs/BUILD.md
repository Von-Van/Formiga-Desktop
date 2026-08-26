# Building Formiga

## Development

Rust 1.97.1 is pinned by `rust-toolchain.toml`.

```sh
cargo test --workspace
cargo run -p formiga-tools -- contact-sheet --output contact-sheet.png
cargo run -p formiga-tools -- animation-preview --seed 17 --output animation-preview.png
cargo run -p formiga-tools -- simulate 181
cargo run -p formiga-desktop
```

The desktop binary is supported on macOS 14+ and Windows 10/11 x64. The simulation, art, and save
tests are platform-independent.

## macOS bundle

Run `bash scripts/package-macos.sh` on macOS. It creates a universal ad-hoc-signed app and ZIP in
`dist/`. Set `FORMIGA_CODESIGN_IDENTITY` to a Developer ID Application identity for a distribution
build. Notarization requires the release owner's Apple credentials and is intentionally not embedded
in the repository.

## Windows package

Run `./scripts/package-windows.ps1` in PowerShell on Windows. With WiX 4 installed, it creates a
per-user MSI and portable ZIP in `dist/`; use `-SkipInstaller` for the ZIP only. Authenticode signing
requires the release owner's certificate and is intentionally left as a release-secret step.

GitHub Actions tests both OS implementations and creates unsigned internal artifacts on every run.
