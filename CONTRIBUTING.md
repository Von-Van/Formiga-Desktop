# Contributing

Formiga welcomes focused bug fixes, platform compatibility reports, procedural-art improvements, and
tests. Open an issue before large product or architecture changes.

Before submitting a change:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Platform integration changes should include the relevant manual matrix result in
`docs/TEST_MATRIX.md`. Procedural-art changes should regenerate the contact sheet and pass the
1,000-genome render test. Persistence changes require an explicit migration and round-trip test.

Do not introduce telemetry, global input hooks, Accessibility, Screen Recording, Input Monitoring,
administrator requirements, or application-content inspection without a separately reviewed privacy
and product proposal.
