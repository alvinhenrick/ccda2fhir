# Releasing

Releases are automated with [release-plz](https://release-plz.dev). The six library and tool crates
share one version and are published to crates.io together.

## How a release happens

1. Pull requests are squash-merged to `main` with a conventional title (`feat: ...`, `fix: ...`; see
   CONTRIBUTING).
2. On every push to `main` the **Release-plz** workflow opens or updates a **release pull request**. It
   bumps the version from the commits since the last release (while the version is 0.x, `feat` bumps the
   minor and `fix` the patch; a breaking change also bumps the minor) and adds them to `CHANGELOG.md`.
3. Read that pull request, then merge it. The workflow then publishes the crates to crates.io in
   dependency order, creates the `vX.Y.Z` tag and the GitHub release.

Nothing is published until a release pull request is merged, and a published version is permanent (it can
be yanked, never deleted or reused), so that merge is the one that matters.

## Setup (once)

- A crates.io API token with the `publish-new` and `publish-update` scopes, saved as the repository secret
  `CARGO_REGISTRY_TOKEN`.
- Settings > Actions > General > Workflow permissions > **Allow GitHub Actions to create and approve pull
  requests**.
- Optional later: switch to crates.io Trusted Publishing (no long-lived token) once the crates exist.

## Notes

- `ccda2fhir-wasm` is not released; it is built for the documentation site.
- Pull requests opened by the workflow with the default token do not start other workflows, so CI does not run on
  the release pull request itself. Its content is only version numbers and the changelog; `main` was already
  green. A personal access token or GitHub App token would lift that, if you want CI there.
- The first run published 0.1.0, because it was not on crates.io yet.
