# Building Formiga

Rust 1.97.1 is pinned by `rust-toolchain.toml`, together with rustfmt and Clippy, so `rustup`
installs everything the checks below need on first use. The macOS app also needs the Xcode Command
Line Tools, and Windows needs the Visual Studio C++ build tools.

## Development checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p formiga-tools -- simulate 181
cargo run -p formiga-tools -- generation-sheet
cargo run -p formiga-tools -- classic-sheet
cargo run -p formiga-tools -- habit-sheet
cargo run -p formiga-tools -- home-yard-sheet
cargo run -p formiga-tools -- village-palette-sheet
FORMIGA_DATA_DIR=/tmp/formiga-dev cargo run -p formiga-desktop
```

Set `FORMIGA_DATA_DIR` to run a development build against a scratch colony. It replaces the
platform application-data directory outright, so `colony.json`, its backup and recovery copies, the
rotating `logs/`, and `updates.json` all go there instead. Use it whenever you are trying a change:
a behaviour experiment, a migration, a reset, or a crash mid-write cannot then touch the colony you
actually live with. An empty value is ignored, so `FORMIGA_DATA_DIR= cargo run …` is the real
directory again.

The desktop binary supports macOS 14+ and Windows 10/11 x64. Core simulation, art, habitat,
persistence, drag-state, and occlusion-region tests are platform-independent. Native overlay and
proxy behavior still requires the manual OS matrix in `TEST_MATRIX.md`.

Windows cannot be fully checked from a Mac. `cargo check --target x86_64-pc-windows-gnu` compiles
the core and art crates, but the desktop crate stops in the `ring` dependency, whose build script
needs a MinGW or Windows SDK C toolchain that macOS does not have. The `build-and-test` workflow
runs formatting, Clippy, and the tests on Windows for every push to `main` and every pull request,
so get the change onto one of those and let it pass before tagging a release; a published tag is
awkward to take back. A branch pushed on its own is not built.

## Performance tools

```sh
cargo run --release -p formiga-tools -- tick-bench
scripts/measure-macos.sh --state "four moving, busy desktop" --build "v0.58.0" --colony 4
```

`tick-bench` times `World::tick` over deterministic synthetic desktops with one and four creatures,
so running it before and after a change is a like-for-like comparison of the simulation's own cost.
It must be built in release to mean anything. `measure-macos.sh` samples a running copy of the app
for the procedure in `PERFORMANCE.md` and prints a row for that document's table; it needs no
sudo and no dependencies. The Windows equivalents are `Get-Counter '\Process(formiga)\% Processor
Time'` for CPU and `Get-Process formiga | Select WorkingSet64` for resident memory.

## Review sheets and documentation images

`formiga-tools` draws every reference image in the repository. These commands regenerate all of
them, in place:

```sh
cargo run -p formiga-tools -- hero-image
cargo run -p formiga-tools -- demo-animation
cargo run -p formiga-tools -- contact-sheet --output docs/assets/contact-sheet.png
cargo run -p formiga-tools -- generation-sheet
cargo run -p formiga-tools -- classic-sheet
cargo run -p formiga-tools -- animation-preview --seed 17 --output docs/assets/animation-preview.png
cargo run -p formiga-tools -- expression-sheet
cargo run -p formiga-tools -- gesture-sheet
cargo run -p formiga-tools -- habit-sheet
cargo run -p formiga-tools -- activity-sheet
cargo run -p formiga-tools -- ambient-sheet
cargo run -p formiga-tools -- shelter-sheet
cargo run -p formiga-tools -- village-palette-sheet
cargo run -p formiga-tools -- home-yard-sheet
cargo run -p formiga-tools -- prop-sheet
cargo run -p formiga-tools -- ui-sheet
cargo run -p formiga-tools -- creature-card
cargo run -p formiga-tools -- colony-card
cargo run -p formiga-tools -- postcard-sheet
cargo run -p formiga-tools -- sticker --clip wave --scale 8
cargo run -p formiga-tools -- social-preview
cargo run -p formiga-tools -- itch-cover
cargo run -p formiga-tools -- app-icon
```

Without `--output`, each subcommand writes to its own file under `docs/assets/`, except
`itch-cover` (`packaging/itch/cover.png`) and `app-icon` (the icons in `packaging/shared/`).
`contact-sheet` and `animation-preview` are the exceptions: they default to a scratch file in the
repository root, which Git ignores, so pass `--output` to replace the documentation copy. Every
image is deterministic, so regenerating one that nothing has changed gives back identical pixels.

Some of the sheets are review tools first and illustrations second. `ui-sheet` draws the whole
interface atlas: every thought bubble, menu frame, icon state, and label tab. `prop-sheet` draws
the eight toys, four snacks, and three kinds of drinkware in the paws and mouths that hold them,
and `gesture-sheet` and `habit-sheet` show every pose on every body. `sticker` takes
`--clip walk|wave|cheer|play|snack|sleep|dance`, `--scale 4|8`, and `--seed NUMBER`. `postcard`
draws a single postcard (`--scene nap|picnic|play|dusk`, `--caption TEXT`). `social-preview` is a
1280×640 link-preview image, and `itch-cover` is a 630×500 store cover. Run the tools without a
subcommand for the full list.

## Distribution kit

`packaging/` holds the material a release needs beyond the built binaries, none of which is
published automatically:

- `packaging/windows/winget/` — template manifests for the winget package `VonVan.Formiga` (schema
  1.6.0): the version, installer, and en-US locale files, plus a README covering why the installer
  manifest is per-user scope with no `ProductCode` and why it states plainly that the installer is
  unsigned.
- `scripts/winget-manifest.sh <version>` — reads the already-published GitHub release for that
  version, verifies the Windows MSI's SHA-256, and writes filled-in manifests to
  `packaging/windows/winget/out/<version>/`, which is git-ignored. It builds, signs, and uploads
  nothing, and never touches `microsoft/winget-pkgs`; a maintainer copies the output into a pull
  request by hand.
- `packaging/itch/` — `page.md`, a paste-ready itch.io page kit (classification, tags, descriptions,
  and upload steps), and `cover.png`, drawn by `formiga-tools itch-cover`.
- `packaging/REPOSITORY.md` — a suggested repository description and topic list with the `gh repo
  edit` command for the owner to review and run.

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

## Cutting a release

A release is one commit on `main` titled `Release X.Y.Z with …`, followed by an annotated tag
`vX.Y.Z` whose message is that title. The version number appears in:

- the workspace `Cargo.toml`, which every crate inherits, and `Cargo.lock`;
- the README's title, its example download name, and its "New in X.Y.Z" section, which is also
  added to the top of `docs/RELEASE_NOTES.md`;
- the `docs/GENERATION.md` title and the CHANGELOG heading, `## [X.Y.Z] - YYYY-MM-DD`;
- the default version in `scripts/package-macos.sh` and `scripts/package-windows.ps1`;
- the examples in `packaging/itch/page.md`, `packaging/windows/winget/README.md`, and
  `scripts/winget-manifest.sh`.

Then:

1. Regenerate the documentation images if anything they draw has changed, and run the full check.
2. Push the release commit to `main` and wait for `build-and-test` to pass on macOS **and**
   Windows. That run is the only Windows build a change gets before it ships.
3. Tag the commit and push the tag. `release.yml` packages both platforms and publishes an
   unsigned prerelease with eight files: the DMG, the MSI, two portable ZIPs, and a `.sha256` for
   each. The in-app updater sees it from then on.
4. Check that all eight files are on the release and that the DMG matches its checksum. A large
   upload can fail after the others succeed. `gh run rerun <run-id> --failed` repeats only the
   publish job, which is how the 0.57.1 release was completed when its DMG upload failed.

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
