# Contributing

## Branching model

- `main` — stable, always releasable. Only updated by merging `dev` in.
- `dev` — integration branch. All feature/fix branches target this.
- `feature/<short-name>` — new functionality (e.g. `feature/amp-interactive`).
- `fix/<short-name>` — bug fixes (e.g. `fix/dark-mode-picture-fallback`).

Workflow:

```bash
git checkout dev
git pull
git checkout -b feature/my-thing
# ... commits ...
git push -u origin feature/my-thing
# open a PR into dev
```

`dev` gets merged into `main` for releases, once `cargo test` and `cargo clippy --all-targets -- -D warnings` are clean.

## Before opening a PR

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Both run in CI on every PR; please make sure they pass locally first.
