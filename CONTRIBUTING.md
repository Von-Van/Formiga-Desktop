# Contributing and making your own version

Formiga is a personal portfolio project rather than an openly contributed community project. I am
not currently accepting pull requests, feature submissions, or requests to maintain changes in this
repository. Unsolicited contributions may be closed without review.

You are warmly encouraged to use the code as a starting point for your own experiments, creatures,
and desktop ecosystems. Fork it, remix it, improve it, or take it in a completely different direction
under the terms of the [MIT License](LICENSE).

If you publish a version of your own, please:

- Preserve the license and copyright notice.
- Give the project a distinct name and icon so people do not mistake it for an official Formiga
  release.
- Clearly describe it as an independent fork or derivative.

## Working on your version

Before sharing a build, run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Platform integration changes should be checked against the relevant cases in
`docs/TEST_MATRIX.md`. Procedural-art changes should regenerate the contact sheet and pass the
1,000-genome render test. Persistence changes should include an explicit migration and round-trip
test.

Formiga intentionally avoids telemetry, global input hooks, Accessibility, Screen Recording, Input
Monitoring, administrator requirements, and application-content inspection. Forks are encouraged
to preserve those privacy-friendly defaults and to explain clearly if they choose a different model.
