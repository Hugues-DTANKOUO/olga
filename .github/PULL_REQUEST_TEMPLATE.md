## Summary

<!-- 1–3 sentences: what changed and why. -->

## Type of change

<!-- Tick all that apply. -->

- [ ] Bug fix (non-breaking)
- [ ] New feature (non-breaking)
- [ ] Breaking change (callers will need to update)
- [ ] Performance improvement
- [ ] Refactor (no behaviour change)
- [ ] Documentation
- [ ] Tests
- [ ] CI / build

## Related issues

<!-- e.g. "Fixes #123", "Refs #456", or "n/a". -->

Fixes #

## What changed

<!-- Bullet the substantive changes. Keep it scannable — the diff carries
the rest. Mention the affected modules / formats so reviewers know where
to look. -->

-
-

## How it was tested

<!-- Concrete evidence the change works. Cite tests, corpus samples,
benchmarks — not "it works on my machine". -->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --all-targets`
- [ ] (if Python touched) `ruff check`, `ruff format --check`, `mypy --strict`, `pytest`
- [ ] (if behaviour changed) regression test added or existing one updated

## Public API impact

<!-- Tick one. If "yes", explain in the summary above. -->

- [ ] No public API change
- [ ] New public surface (additive)
- [ ] Breaking change to public API

## Documentation

- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] Doc comments / README / `docs/` updated where relevant
- [ ] Examples in `olgadoc/examples/` still build (if Python surface changed)

## Notes for the reviewer

<!-- Anything non-obvious: trade-offs, follow-ups, deferred work, areas
that deserve extra eyes. Optional. -->
