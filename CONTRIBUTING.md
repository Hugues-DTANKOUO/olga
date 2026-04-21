# Contributing to Olga

Thanks for taking the time to improve Olga. This document captures the
conventions that keep the codebase coherent and the review loop fast.

## Repo layout

```
olga/                     # Rust workspace root
├─ src/                   # Engine crate (PDF / DOCX / XLSX / HTML)
├─ tests/                 # Cargo integration tests + corpus fixtures
├─ benches/               # Criterion benchmarks
├─ docs-dev/              # Architecture notes, ADRs, design memos
├─ docs/                  # Public documentation site (MkDocs)
└─ olgadoc/               # Python bindings crate (PyO3 + maturin)
   ├─ src/                # Rust glue for the PyO3 module
   ├─ python/olgadoc/     # Python package, TypedDicts, stubs
   ├─ tests/              # pytest test suite
   └─ examples/           # Runnable examples
```

## Development environment

### Rust

```bash
rustup toolchain install 1.88   # pinned in Cargo.toml (rust-version)
cargo build --workspace
cargo test --workspace --exclude olgadoc
cargo clippy --workspace --exclude olgadoc --all-targets -- -D warnings
cargo fmt --all -- --check
```

`olgadoc` is excluded from plain `cargo test` because the
`extension-module` PyO3 feature can't link the CPython runtime outside a
maturin build. The Python test suite exercises it end-to-end.

### Python

From `olgadoc/`:

```bash
python -m venv .venv
source .venv/bin/activate
pip install maturin ruff mypy pytest typing_extensions
maturin develop            # builds the Rust extension into the venv
pytest tests/ -q
ruff check python/ tests/ --select F,E,W,I
ruff format --check python/ tests/
mypy --strict python/olgadoc/ tests/
```

## Coding conventions

### Rust

- `rustfmt` defaults, enforced by CI.
- `clippy` with `-D warnings` is the floor. No `#[allow(...)]` without a
  comment that points at a tracked issue.
- Errors go through the crate's `thiserror`-backed enums. Panics must
  only escape in genuinely unreachable branches and carry an `expect`
  string that names the invariant.
- Prefer small, composable modules. Cross-format logic belongs in the
  core pipeline; format-specific logic stays under its format module.

### Python

- `from __future__ import annotations` at the top of every module.
- Type-annotate every function and method; no untyped arguments or
  returns. `mypy --strict` must pass on `python/olgadoc/` and `tests/`.
- No `Any` on the public surface. Use `TypedDict` with
  `Required[...]` / `NotRequired[...]`, `Literal[...]` for
  discriminators, and `Union[...]` for closed unions. Runtime
  `TypedDict` classes belong in `olgadoc/__init__.py` so IDEs and
  `inspect` both see them.
- `ruff check --select F,E,W,I` and `ruff format` clean. Line length is
  88 characters.
- Docstrings use the Google style (`Args:` / `Returns:` / `Raises:`
  blocks). Tests use Gherkin-style docstrings (`GIVEN` / `WHEN` /
  `THEN`).

## Testing

- **Rust**: unit tests next to the code, integration tests in `tests/`,
  proptest generators in `tests/model/`. Benchmarks in `benches/`.
- **Python**: focused unit tests in `tests/test_document.py`,
  `tests/test_processability.py`, `tests/test_typing.py`; cross-format
  guarantees in `tests/test_e2e.py`. Static-typing smoke in
  `tests/_typing_consumer.py` (ignored by pytest).
- New behaviour needs a test. Regressions need a regression test.

## Pull requests

1. Open a branch off `main`.
2. Keep commits scoped — one logical change per commit, subject under
   72 characters, imperative mood.
3. Run the full Rust + Python check loop locally before opening the PR.
4. In the PR description: what changed, why, any follow-ups. Link the
   issue if there is one.
5. CI must be green before review. Reviewers will focus on the "why"
   and on edge cases — prove the "what" with the test diff.

## Security

If you discover a security issue, please do not open a public issue.
Email the maintainers at the address listed in `Cargo.toml`'s
`repository` metadata so we can coordinate a fix before disclosure.

## License

By contributing you agree that your contributions are licensed under
the [Apache License, Version 2.0](https://github.com/Hugues-DTANKOUO/olga/blob/main/LICENSE) like the rest of the project.
Per section 5 of the Apache License, submissions you intentionally
offer for inclusion are automatically provided under those same terms
unless you explicitly state otherwise; no separate CLA is required.
