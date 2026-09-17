# Building Formiga

Rust 1.97.1 is pinned by `rust-toolchain.toml`.

## Development checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p formiga-tools -- simulate 181
cargo run -p formiga-tools -- generation-sheet
cargo run -p formiga-tools -- home-yard-sheet
cargo run -p formiga-desktop
```

The desktop binary supports macOS 14+ and Windows 10/11 x64. Core simulation, art, habitat,
persistence, drag-state, and occlusion-region tests are platform-independent. Native overlay and
proxy behavior still requires the manual OS matrix in `TEST_MATRIX.md`.

Windows cannot be fully checked from a Mac. `cargo check --target x86_64-pc-windows-gnu` compiles
the core and art crates, but the desktop crate stops in the `ring` dependency, whose build script
needs a MinGW or Windows SDK C toolchain that macOS does not have. The `build-and-test` workflow
runs formatting, Clippy, and the tests on Windows, so push a commit and let it pass before tagging
a release; a published tag is awkward to take back.

## Performance tools

```sh
cargo run --release -p formiga-tools -- tick-bench
scripts/measure-macos.sh --state "four moving, busy desktop" --build "v0.57.1" --colony 4
```

`tick-bench` times `World::tick` over deterministic synthetic desktops with one and four creatures,
so running it before and after a change is a like-for-like comparison of the simulation's own cost.
It must be built in release to mean anything. `measure-macos.sh` samples a running copy of the app
for the procedure in `PERFORMANCE.md` and prints a row for that document's table; it needs no
sudo and no dependencies. The Windows equivalents are `Get-Counter '\Process(formiga)\% Processor
Time'` for CPU and `Get-Process formiga | Select WorkingSet64` for resident memory.

## Downloads for nontechnical users

Tagged builds publish four ready-to-run files on GitHub Releases. Recommend the macOS DMG and
Windows MSI; the ZIP files are portable alternatives. Opening Formiga for the first time creates a
colony and opens Settings automatically. No Rust installation, command line, or separate runtime is
needed.

The release tag is the package source of truth and must be a full three-part semantic version:
the updater parses the tag with `semver`, so `v0.36.6` is accepted and `v0.36` is rejected. A tag
`v0.36.6` builds an application that reports version `0.36.6`, places the same version in the macOS
bundle and Windows MSI, and publishes the exact updater-compatible names
`Formiga-0.36.6-macOS-universal.dmg` and `Formiga-0.36.6-windows-x64.msi`. Do not rename these two
assets after publishing. Their companion `.sha256` files are the fallback verification source when
GitHub release metadata does not provide a digest.

## macOS universal preview

Run `bash scripts/package-macos.sh` on macOS. It creates an arm64/x86_64 universal app, ad-hoc signs
it when no identity is supplied, and writes a drag-to-Applications DMG, ZIP, and SHA-256 checksums
to `dist/`.

Set `FORMIGA_CODESIGN_IDENTITY` to a Developer ID Application identity for distribution signing
with the hardened runtime and a timestamp. Set `FORMIGA_NOTARY_PROFILE` to a `notarytool`
keychain profile to notarize the DMG and staple the ticket to it. The credentials themselves live
only in the release owner's keychain, created once with `xcrun notarytool store-credentials`; none
are stored in the repository. Both need an Apple Developer Program membership. The release workflow
passes neither today, so tagged builds are ad-hoc signed.

Unsigned/ad-hoc preview build: after downloading, Control-click the app and choose **Open**. Do
not advise users to disable Gatekeeper globally.

## Windows preview

Run `./scripts/package-windows.ps1` in PowerShell. With WiX 4 installed it creates a per-user MSI
with Desktop and Start-menu shortcuts, a portable ZIP, and SHA-256 checksum files. Use
`-SkipInstaller` for the ZIP only.

Unsigned preview build: Windows SmartScreen may show **More info → Run anyway**. Set
`FORMIGA_SIGNTOOL_CERT_SHA1` to the thumbprint of a code-signing certificate in the Windows
certificate store and the script signs the executable and the MSI with a timestamp. Code-signing
certificates now require identity validation and keys held in hardware or a cloud signing service,
and a newly signed publisher still meets SmartScreen warnings until it builds reputation.

GitHub Actions checks both operating systems and packages unsigned preview artifacts. Tags matching
`v*` feed the release workflow. The current workflow marks preview builds as prereleases; the app
intentionally checks those releases as well as stable releases.
