# Contributing

Thanks for helping. A few rules keep a clinical-data converter trustworthy.

## Before you open a pull request
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass.
- Every mapping change has a test. Prefer a small, synthetic C-CDA fixture that shows the case.
- **Never commit real patient data.** Fixtures must be synthetic or from public HL7/ONC sample sets, with the source noted.
- **Never put document values in logs, errors or report messages.** Use template ids, codes and counts only.

## Mapping changes
- Cite the source: the IG mapping row, the C-CDA template id, the US Core profile element.
- If the IG is silent or ambiguous, say what you chose and why in the PR. Decisions go in `docs/decisions/`.
- Lossy mappings must emit a `Report` issue with a stable code. Never drop data silently.

## Style
- No `unsafe` (the workspace forbids it). Avoid `unwrap`/`expect`/`panic` outside tests; return errors or report issues.
- Keep the core crates free of I/O and global state.
- Small PRs, one concern each.

## Commits and pull request titles
Pull requests are squash-merged, so the **PR title is the commit message** and drives releases
(see `docs/releasing.md`). Start it with a type:

- `feat:` new behaviour (a minor bump while the version is 0.x)
- `fix:` a bug fix (a patch bump)
- `docs:`, `test:`, `chore:`, `ci:`, `refactor:`, `perf:` no release by themselves
- add `!` (`feat!:`) or a `BREAKING CHANGE:` line in the body for an incompatible change

Then a short imperative summary. Explain the why in the body when it is not obvious.
