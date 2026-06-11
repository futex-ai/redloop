# Redloop CLI Release

`redloop-cli` releases are private GitHub Releases from the Redloop repository.
The CLI uses the shared release protocol implemented in
`https://github.com/futex-ai/cli-release`.

## Release Server

- GCP project: `internal-498318`
- Cloud Run service: `redloop-cli-release`
- Region: `europe-west2`
- Default URL:
  `https://redloop-cli-release-993259844560.europe-west2.run.app`
- Auth: unauthenticated HTTP for CLI clients
- Minimum instances: `0`
- GitHub token secret: `redloop-cli-release-github-token`
- Binary allowlist: `redloop-cli`

The service is public so installed CLIs can fetch metadata without Google IAM
credentials. Private GitHub Release assets remain protected by the server-side
GitHub token stored in Secret Manager.

## Release Workflow

The `Redloop CLI Release` workflow:

1. Runs release-plz for the Redloop workspace.
2. Builds `redloop-cli` for Linux and macOS release targets.
3. Packages archives with `scripts/cli-release.sh package`.
4. Uploads archives and `.sha256` files to the private GitHub Release.
5. Checks out `futex-ai/cli-release`.
6. Builds the shared `cli-release-server` image.
7. Pushes immutable version and commit-SHA tags to Artifact Registry in
   `internal-498318`.
8. Deploys the pinned image to Cloud Run.
9. Smoke-tests release metadata routes.

Required GitHub secrets:

- `CI_GITHUB_FUTEX_SHARED` - read access to `futex-ai/cli-release` for
  Cargo's private `cli-updater` git dependency and shared release-server
  checkout. The token must be exposed to the `futex-ai/redloop` repository and
  must have read-only Contents access to `futex-ai/cli-release`.
- `GCP_WORKLOAD_IDENTITY_PROVIDER` - workload identity provider for GCP deploys.
- `GCP_SERVICE_ACCOUNT` - deploy service account email.

Release workflows configure Cargo with `scripts/configure-private-cargo-git.sh`.
Those workflows pass `CI_GITHUB_FUTEX_SHARED` to that script as
`CLI_RELEASE_REPO_TOKEN`; the script then sets Git URL rewrites for the private
dependency and runs `git ls-remote` against `futex-ai/cli-release` before Cargo
starts, so token access failures surface before the Rust build.

Required GCP setup:

- Artifact Registry Docker repository: `redloop-cli-release`
- Secret Manager secret: `redloop-cli-release-github-token`
- Deploy service account permissions for Artifact Registry, Cloud Run, and
  Secret Manager secret binding.
- Runtime Cloud Run service account access to
  `redloop-cli-release-github-token`.

## Upgrade Client

`redloop-cli upgrade status` checks:

```bash
redloop-cli upgrade status
```

Use an alternate release server for smoke tests:

```bash
redloop-cli upgrade status \
  --release-server https://redloop-cli-release-993259844560.europe-west2.run.app
```

The `REDLOOP_RELEASE_SERVER_URL` environment variable also overrides the
embedded default.

## Rollback

To roll back the release server, redeploy a previous immutable Artifact
Registry tag to the same Cloud Run service. To roll back a CLI release, mark the
bad GitHub Release as draft or remove its Redloop CLI assets, then publish a
fixed release tag.
