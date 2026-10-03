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

## Trying a trip to Formiga Hill

Formiga Hill is a separate application, but Desktop's side of a trip can be tried without it.
`formiga-hill-stub` stands in for Hill: it reads the snapshot it is handed, checks it, prints who
arrived, says it can host them, waits, and writes a receipt, all without a window.

```sh
cargo build -p formiga-tools --bin formiga-hill-stub
FORMIGA_DATA_DIR=/tmp/formiga-dev \
FORMIGA_HILL_PATH="$PWD/target/debug/formiga-hill-stub" \
FORMIGA_HILL_TRIP_AFTER=5 \
FORMIGA_HILL_STUB=stay=20 \
cargo run -p formiga-desktop
```

`FORMIGA_HILL_PATH` names a Hill executable (or, on macOS, an `.app`) to use instead of looking for
an installed one, and makes the tray offer the trip. `FORMIGA_HILL_TRIP_AFTER` sends the colony
that many seconds after launch, once, as if the tray had been used; it is ignored unless
`FORMIGA_HILL_PATH` is set too. `FORMIGA_HILL_STUB` takes a comma-separated list of how the stub
should behave: `stay=SECONDS`, `refuse=version|busy|invalid`, `crash`, `silent`, `garbage`,
`stranger`, `souvenir` to bring home every souvenir the snapshot says Desktop keeps (the Journal
then shows them under Souvenirs), `souvenir=ID` to bring home just that one, listed or not, and
`sheet=PATH` to draw every traveler into a PNG. Each trip's files are under `travel/` in the
scratch data directory while it is open, and the log names each step.

The travel contract has its own fixtures. `crates/formiga-travel/tests/fixtures` holds every
version as it shipped; the tests fail if a build stops reading any of them, or stops writing its
own version byte for byte. When a new travel version is deliberately added,
`FORMIGA_TRAVEL_BLESS=1 cargo test -p formiga-travel --test golden` writes that version's files
beside the others, named for it, and never touches a version that has shipped.

The Windows half of Hill discovery, which reads the registry, can be type-checked from a Mac
without the desktop crate's `ring` dependency by compiling `platform/hill.rs` on its own against
`formiga-travel` and the `windows` crate for `x86_64-pc-windows-msvc`.

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

## Long simulated runs

```sh
taskpolicy -b cargo run --release -p formiga-tools -- soak --colonies 40 --days 2 --threads 2
cargo run --release -p formiga-tools -- soak --seed 7 --only 514 --days 3
```

`soak` lives many randomized colonies through days of sessions and gaps: twenty ticks a second
while Formiga would be running, and the clock jumping ahead overnight, across weekends and through
coffee breaks. Between sessions displays are plugged in, unplugged and rescaled, preferences are
toggled, a weekly routine is followed, the village is rearranged and changes are undone, companions
arrive and leave, and the colony is written to disk and read back the way a relaunch reads it.
During a session windows open, move and close, the cursor wanders, and companions are petted,
carried, tossed and offered things. At every checkpoint each colony must hold nothing
`formiga_core::violations` names, every companion must be somewhere a display reaches, a weekly
routine must be the one the week says it is, and a colony read back must be the colony written.
`--damage N` also damages each colony's last file N ways and reads it back. A colony that fails
writes `soak-failure-<seed>-<colony>.json`, with its last good save, to `--out`, and the run prints
the command that replays exactly that life.

It uses half the machine's cores unless `--threads` says otherwise; on a Mac someone is working at,
run it small and under `taskpolicy -b`, which keeps it on the efficiency cores. The `soak` workflow
runs it in full every night — 1,500 colonies of three simulated days each, in each of three time
zones (UTC, Pacific/Auckland and America/Los_Angeles), so local midnights and weekly routines fall
at different moments — and uploads any failure files. It can also be started by hand from the
Actions tab with other numbers and a seed.

## Review sheets and documentation images

`formiga-tools` draws every reference image in the repository. These commands regenerate all of
them, in place:

```sh
cargo run -p formiga-tools -- hero-image
cargo run -p formiga-tools -- demo-animation
cargo run -p formiga-tools -- contact-sheet --output docs/assets/contact-sheet.png
cargo run -p formiga-tools -- generation-sheet
cargo run -p formiga-tools -- classic-sheet
cargo run -p formiga-tools -- face-sheet
cargo run -p formiga-tools -- temperament-sheet
cargo run -p formiga-tools -- animation-preview --seed 17 --output docs/assets/animation-preview.png
cargo run -p formiga-tools -- expression-sheet
cargo run -p formiga-tools -- gesture-sheet
cargo run -p formiga-tools -- habit-sheet
cargo run -p formiga-tools -- activity-sheet
cargo run -p formiga-tools -- ambient-sheet
cargo run -p formiga-tools -- shelter-sheet
cargo run -p formiga-tools -- decoration-sheet
cargo run -p formiga-tools -- village-palette-sheet
cargo run -p formiga-tools -- home-yard-sheet
cargo run -p formiga-tools -- prop-sheet
cargo run -p formiga-tools -- wonder-sheet
cargo run -p formiga-tools -- train-sheet
cargo run -p formiga-tools -- motion-sheet
cargo run -p formiga-tools -- accessory-sheet
cargo run -p formiga-tools -- village-life-sheet
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
`itch-cover` (`packaging/itch/cover.png`) and `app-icon` (the icons in `packaging/shared/`, among
them `Formiga-tray.png`, the 64-pixel copy the app embeds for its menu bar and tray icon).
`contact-sheet` and `animation-preview` are the exceptions: they default to a scratch file in the
repository root, which Git ignores, so pass `--output` to replace the documentation copy. Every
image is deterministic, so regenerating one that nothing has changed gives back identical pixels.

`cuteness-sheet` is for judging the generator rather than illustrating it. It draws 150 freshly
generated companions (`--count` takes 1 to 400) on a numbered sheet, to `cuteness-sheet.png` in the
repository root unless `--output` says otherwise, and prints a table of what each is made of: how
it was chosen (cute, weird-cute or oddball), its archetype, coherence score, size, body, ears,
tail, face layout and markings. Each companion stands beside the same village tree, as on the
generation sheet, so their sizes can be compared. `--seed NUMBER` draws a different sheet, `--edition archetypes` draws what the
generator before 0.63.0 makes from the same seeds, without details, and `--edition original` what
the one before 0.62.0 makes. Rate them in a text file of lines like
`good: 1 4 9`, `ok: 2, 3` and `bad: 12`, pass it back with `--ratings FILE`, and the report shows
which archetypes, faces and parts turn up among the ones rated bad more often than they turn up at
all.

Some of the sheets are review tools first and illustrations second. `decoration-sheet` draws every
house decoration on its own, on a companion's cottage of each type — the narrowest house, where a
decoration has the least room — in blocks by the place on a house it goes, and the four that light
up, lit after dark; it is the sheet to judge a decoration's silhouette by. `ui-sheet` draws the whole
interface atlas: every thought bubble, menu frame, icon state, and label tab. `prop-sheet` draws
the colony's whole keepsake sheet on pale, dark and busy wallpaper, sixteen keepsakes held up the
way the overlay holds them, and the toys, snacks and drinkware in the paws and mouths that hold
them. `wonder-sheet` draws every wonder's frames and its script played out at six moments, alone and for
two, by the reference companions; `motion-sheet` draws every body's walk, run, rest, meal and sleep
loops frame by frame, where a loop that hitches or repeats itself shows. `accessory-sheet` draws everything a companion can wear on six kinds of body, resting,
walking, asleep and cheering. `village-life-sheet` draws the village getting on with things, a
moment to a cell — the gardens tended, every kind of house seen to, somebody indoors and somebody
on a roof, the three mishaps, and a yawn going round — composed the way the overlay composes them.
`face-sheet` draws the twelve face layouts a new companion can wear in every expression and a
blink, and `temperament-sheet` every frame of the six poses a temperament strikes, each with the
face its moment wears. `shelter-sheet` draws every kind of house plain, dressed two ways
between which every decoration appears, as a cottage by day, with its resident at home and lit
after dark, and with its resident sitting on the roof where the simulation seats it. And
`gesture-sheet` and `habit-sheet` show every pose on every body. `sticker` takes
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
   Windows. That run is the only Windows build a change gets before it ships; what has been tried
   by hand on Windows hardware is recorded in `docs/TEST_MATRIX.md`.
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
