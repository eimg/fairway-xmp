# Build and release

Builds and publication are separate actions. A successful automated candidate is ready to publish when the owner authorizes it. Device testing is optional. There is no manual acceptance JSON, test-attestation phase or prerelease scope freeze.

## Build workflows

Run **Release candidate** (`release-candidate.yml`) from the intended commit, or push a `candidate-*` tag. It combines browser checks and the Windows, Apple Silicon macOS and Linux installer workflows at the same source commit. Individual platform workflows can also produce development installers or standalone test applications without publishing a release.

The macOS candidate uses `build` followed by `package`; signing, notarization and updater signing happen in `package`. The separate `standalone` and `sign-standalone` jobs run only when manually selecting `standalone_only` to produce a permissions-test app ZIP. Their skipped status in a normal candidate is expected.

The candidate records application/runtime versions, source identity, installer/updater hashes, automated lifecycle results and signing status in `candidate-manifest.json`. It does not generate a device checklist or publish anything. The metadata artifact retains its existing internal name for compatibility with previously built candidates.

Application versions must agree in `package.json`, both root entries in `package-lock.json`, the package in `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`. Check them with:

```sh
python3 scripts/release_candidate.py version
```

Local builds are covered in [Contributing](../CONTRIBUTING.md). See [CI performance](ci-build-performance.md) for caching and retries. Publish the files from one successful run; a changed application/runtime needs a new build. Release notes and documentation can change without rebuilding installers.

## Runtime sources and redistribution

`runtime-manifest.json` defines versions and PHP extension requirements. Desktop downloads, Linux downloads and corresponding sources are pinned in `runtime-downloads.lock.json`, `runtime-linux-downloads.lock.json` and `runtime-sources.lock.json`. Runtime preparation stages files without installing system services on target machines.

On the Mac source collector, after preparing the runtime:

```sh
python3 scripts/package_sources.py --locked
```

The archive is written under `artifacts/sources-VERSION/`. It includes the application source, runtime build recipes, patches, component licenses and source trees corresponding to the native libraries. Windows dependency sources come from the pinned PHP distributions' SBOMs. Linux archives include exact Ubuntu source packages and distribution patches for copied libraries. Use `--source-ref BUILD_COMMIT` when collecting sources from a newer checkout for an existing build.

All platform source artifacts include fnm’s unmodified tag and locked Rust crate archives with an offline reconstruction recipe. Node itself is not bundled. Mac/Windows candidate jobs run isolated native fnm activation checks; Linux checks the native fnm executable and optional Node LTS downloads with Node/npm/npx in an isolated store.

Use `--refresh-lock` only when deliberately updating runtime sources; review and commit the resulting source lock. Locked packaging rejects new inputs. A Gitiles snapshot may be recompressed only if its full pinned Git tree still matches every file, symlink and executable mode. The output inventory records the actual archive checksum. Distribute the corresponding-source archives beside binaries as required by bundled licenses; an upstream repository link does not replace them.

## Signing

Mac distribution builds sign all Mach-O code, notarize/staple the app, then create and separately sign/notarize/staple the installer. Only PHP interpreters receive the PCRE JIT entitlement. Bundled libraries and application executables keep their normal entitlements. Signed PHP checks exercise regex JIT, PostgreSQL clients and Composer in every bundled version.

Windows installers currently lack Authenticode signing. Desktop updater payloads are separately signed with the public key in `packaging/updater.pub`; these signatures are required independently of platform distribution signatures. Linux updates are manual.

| Credential | Purpose |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Version-bound desktop update signatures |
| `APPLE_CERTIFICATES_P12`, `APPLE_CERTIFICATES_PASSWORD` | Release application/installer identities |
| `APPLE_API_PRIVATE_KEY`, `APPLE_API_KEY`, `APPLE_API_ISSUER` | Apple notarization |

Keep private keys and exported identities outside source and artifacts. The configured Apple team is `4Q27NQ2UAF`; local signing can use the validated `fairway-notary` profile. `scripts/package_macos.sh` accepts `APPLE_SIGNING_IDENTITY`, `APPLE_INSTALLER_IDENTITY` and `APPLE_NOTARY_PROFILE`.

Desktop compilation and signing use separate runners. Builders have no signing credentials; packaging imports credentials only for signing, disables npm lifecycle scripts, restores no build caches, and removes the temporary Apple keychain before executing bundled code. `FAIRWAY_DEFER_RUNTIME_CHECKS` and `FAIRWAY_DEFER_UPDATER_SIGNATURE` keep bundled execution and updater signing out of the Apple-credential step. Updater keys are supplied only to the direct signing command.

Signed commits must match the build and already be in fetched `origin/main` history. Standalone comparison refs are validated before checkout. Unsigned branch builds remain available. Repository write permission belongs only to publication jobs; the updater verifier is compiled in a separate read-only job. These source/signature checks do not impose a feature freeze or a device-test requirement.

## Publish

The prepared release notes are in [release-notes.md](release-notes.md); use `docs/release-notes.md` as the publication workflow's notes path.

When the owner authorizes release:

1. Prepare release notes and create/push a version tag at the tested source commit.
2. Dispatch **Publish tested release** (`publish-release.yml`) with the successful candidate run ID, source commit, version tag and notes path.
3. Select update-feed promotion if the owner also wants to offer the release in-app.

The publisher verifies successful CI, source/tag identity, downloaded file hashes, native updater results and both updater signatures. It creates a draft from those exact files, verifies uploaded digests/sizes, then publishes. It does not rebuild. `BUILD-INFO.json` records the explicit publication method/actor without claiming human tests passed. Existing published assets are immutable; subsequent fixes receive a new version.

Stable versions are ordinary GitHub releases marked latest. Preview versions are prereleases. This classification controls delivery, not which features may be developed. The source checkout's current version does not prevent moving to a stable version through the normal build process.

## Update feeds

Installed preview clients use `beta-feed/latest.json`; stable clients use `stable-feed/latest.json`. The legacy preview URL stays valid for installed clients. Preview/stable ordering prevents downgrades and unwanted preview delivery to stable installations. Publishing a stable version can advance both feeds while preserving a newer next-version preview.

Feed promotion is explicit and off by default. It runs after the numbered release exists. If promotion fails, the release remains available for manual download; retry `scripts/publish_release_feed.py` with `GH_REPO` and `FAIRWAY_TAG`. Do not rebuild or replace published installers to retry metadata delivery.

## Agent skill in source releases

`skills/fairway-xmp` contains the portable Fairway XMP agent skill and optional
agent UI metadata. Keep this directory tracked. GitHub's **Source code** archives
and the existing `git archive` source bundles include the folder with the exact
release commit; there is no separate skill archive to build or upload. Users
extract and copy the whole folder into their agent application's supported skills
directory. The skill operates the installed Fairway XMP application through its CLI. The published guide is `#cli` on
the documentation website, with a For agents block before the human procedure.
