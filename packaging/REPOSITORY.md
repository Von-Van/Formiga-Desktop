# Repository metadata suggestions

The repository currently has no description, no topics, and no homepage set. These are cheap,
reversible wins for discovery - GitHub topics feed search and the "explore" pages, and a good
description is what shows up in search results and link previews. Nothing in this file changes
anything by itself; it is copy and a command for the owner to review and run.

## Suggested description (278 of 350 characters allowed)

```
A privacy-first desktop companion for macOS and Windows: seeded procedural pixel creatures live in transparent overlays, perch on your real windows, react to your cursor, and grow into a four-creature colony. Open source (MIT); everything runs locally, no accounts or analytics.
```

## Suggested topics (14)

All lowercase-hyphenated and well under GitHub's 50-character-per-topic limit:

```
desktop-pet, desktop-companion, virtual-pet, rust, wgpu, egui, pixel-art,
procedural-generation, generative-art, simulation, macos, windows, privacy, offline-first
```

Rationale:

- `desktop-pet`, `desktop-companion`, `virtual-pet` - the three phrasings people actually search
  for this category of app under.
- `rust`, `wgpu`, `egui` - the real implementation stack (see `Cargo.toml`'s workspace
  dependencies), useful for developers browsing by technology.
- `pixel-art`, `procedural-generation`, `generative-art`, `simulation` - what the project
  actually does: seeded, rasterized, simulated creatures, not hand-drawn or AI-generated art.
- `macos`, `windows` - the two supported platforms.
- `privacy`, `offline-first` - the project's own stated selling point, straight from the README
  and `docs/PRIVACY.md`.

## Applying it

This is the exact command that would apply both. It has not been run - review it, adjust to
taste, and run it with an account that has admin access to the repository:

```sh
gh repo edit Von-Van/Formiga-Desktop \
  --description "A privacy-first desktop companion for macOS and Windows: seeded procedural pixel creatures live in transparent overlays, perch on your real windows, react to your cursor, and grow into a four-creature colony. Open source (MIT); everything runs locally, no accounts or analytics." \
  --add-topic desktop-pet,desktop-companion,virtual-pet,rust,wgpu,egui,pixel-art,procedural-generation,generative-art,simulation,macos,windows,privacy,offline-first
```

`gh repo edit --help` also lists a `--homepage URL` flag; not suggested here since itch.io page
and GitHub Pages are both still hypothetical until the owner publishes one; add it later with the
same command shape once one of the two exists, e.g. `--homepage https://vonvan.itch.io/formiga`.
