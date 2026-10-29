# CI and build performance

Routine verification is manual-only: run **Actions → Build and verify Fairway XMP → Run workflow** on the intended branch/ref. Ordinary pushes and pull requests do not start application builds or the full browser suite. Starting another verification on the same ref cancels the older verification. Candidate checks still include every platform and browser review from one commit; no candidate is canceled by routine verification concurrency. The separate documentation website workflow retains its lightweight checks for relevant pull requests and manual publication from `main`.

Browser review and the Docker portability host use the tested Ubuntu 24.04 image explicitly. This keeps their environment stable through GitHub's [announced Ubuntu 26.04 migration](https://github.com/actions/runner-images/issues/14748). Runtime compilation continues to use Ubuntu 22.04 for the glibc 2.35 baseline.

## Runtime and dependency reuse

`Warm build inputs` is manual-only, either through its own **Run workflow** action or the verification workflow's `warm_inputs` checkbox. Runtime, Cargo and workflow changes do not automatically start it. It prepares the locked Windows/macOS/Linux inputs and compiles dependencies with the same Rust cache identity as the installer jobs. It covers debug test/lint profiles plus release app, updater-probe, and installer-probe variants, including Tauri's custom-protocol feature and the explicit Windows target. It does not import signing credentials, assemble candidate manifests, or publish anything. Warm-up failures are independent of routine verification and should be resolved before cutting a candidate.

Mac warm-up and installer jobs read `MACOSX_DEPLOYMENT_TARGET` from Tauri's configured minimum system version before restoring Rust caches. The deployment target participates in the cache key, keeping plain Cargo commands, probes, and Tauri compilation aligned. Application binaries are rebuilt for the current commit; the shared Rust cache retains dependencies rather than old application executables.

GitHub caches are scoped by branch/ref: different candidate tags cannot restore each other's caches, but they can restore `main` caches. Run the warmer on `main` before creating a candidate tag, especially after changing runtime inputs. A manual run on a feature branch warms that branch only. The routine **Build and verify Fairway XMP** workflow also has a manual `warm_inputs` checkbox, useful when validating the warmer on a branch before it reaches `main`. A Rust toolchain update can invalidate compiled dependencies; dispatch the warmer again after such an update. Warm-up is deliberately not a dependency of candidate builds, so a cache miss never blocks a correct fresh build.

Windows and macOS cache `.runtime-cache`, containing downloaded upstream archives. The exact key includes the platform/architecture and current runtime/source/upgrade-fixture locks. Older download caches may supply unchanged archives: every archive consumed by the preparer or source collector is checked against its committed checksum or pinned Git tree before use. Source archives, runtime binaries, versions, compression settings, and signatures are unchanged by caching. No credentials, student data, signing keychains, or generated release installers are cached.

Linux caches its staged glibc 2.35 runtime together with the corresponding sources, avoiding a second large archive cache for the same inputs. The warmer and installer use one composite action, so their preparation commands and keys stay aligned. A change to the runtime manifest, lock, notices, preparers, fnm source reconstruction helper, or composite action invalidates that cache. Every restore runs runtime verification. The builder still installs its compilation prerequisites; target machines need none of those packages.

Linux source setup enables Ubuntu sources and signed, source-only PGDG live/archive repositories. Hosted images can retain PGDG `libpq` packages after removing their repository configuration; the archive supplies older exact versions required by the corresponding-source collector. The builder also refreshes MySQL's `libudev1` dependency and `liblzma` so their installed versions have corresponding sources in the current Ubuntu indexes. Source-availability checks for all three libraries run before PHP compilation. The PGDG entries do not enable additional binary packages or package a PostgreSQL server.

Caches are bounded and GitHub evicts less-recently-used entries. Avoid duplicate multi-gigabyte caches across candidate tags; warm reusable inputs on `main`. Release assets are independent of caches.

Runtime/download caches are saved after successful preparation, before expensive release packaging. Rust dependency caches are published only after a successful job. GitHub cache entries are immutable: saving an incomplete cache during a failed/canceled build would prevent a later successful run from filling it in. The independent warmer seeds complete release dependencies ahead of candidate packaging, so a later signing/lifecycle failure can reuse that completed cache. macOS collects locked sources before importing Apple credentials or notarizing, and desktop jobs compile lifecycle probes and obtain the baseline installer before final packaging. This catches source/network/compilation failures earlier without dropping release checks.

The shared `installer-complete-v1` key names the set of warmed Rust profiles. Bump it in the warmer and all three installers when that set changes; an exact cache hit cannot be extended in place.

Runtime version checks are local. In particular, Mailpit verification uses `version --no-release-check` to compare the bundled version without contacting GitHub for an upstream release lookup. A network failure during that unrelated lookup must not invalidate an otherwise correct locked runtime.

## Signing runner boundary

Desktop compilation and packaging now use separate runners. The builder uploads unsigned inputs and already compiled lifecycle probes; packaging installs locked tools with npm lifecycle scripts disabled, imports credentials only for signing, and runs the existing native acceptance checks after credential cleanup. Packaging does not restore build caches. Compilation caches can therefore be saved after a successful build even when later signing or installation checks fail. The publication signature verifier is likewise built with read-only permissions before its write-enabled publication job.

Intermediate transfer artifacts expire after one day. Re-run failed packaging jobs while their build inputs remain available; rebuild if those inputs have expired. Tar preserves macOS bundle symlinks and executable modes across transfer. The final installer/updater artifacts, source identity and acceptance checks are still generated from the same exact build inputs.

After publication, superseded candidate outputs, duplicate routine UI reports, old diagnostic comparison builds and completed build-transfer artifacts can be removed separately. Retain published-candidate evidence and the latest routine verification report. Do not delete published releases, their source archives/checksums, acceptance records or workflow logs as part of artifact cleanup. Local cleanup can remove unused test bundles, disposable databases, old staging directories and downloads no longer referenced by the current or upgrade-fixture locks; retain useful compiler/toolchain and locked-download caches. No recurring cleanup is configured by this policy.

## Candidate retry rules

For a transient failure at the same commit, use GitHub's **Re-run failed jobs** on the existing candidate run. This preserves successful platform jobs and their exact installer artifacts. For example:

```sh
gh run rerun RUN_ID --failed
```

The failed platform job starts again; this change does not resume an arbitrary step inside a job. Keep using the full candidate run and its assembled manifest for publication. A code, runtime, packaging, or workflow fix requires a new candidate at the new commit; do not combine platform artifacts from different source commits. Publish only the tested files, without rebuilding them for publication.

## Measuring changes

Compare cold and warm runs separately, and inspect step durations rather than adding jobs that ran in parallel. Runtime compilation, installer compression and Apple service time can dominate different runs. Preserve the existing compression settings when comparing download sizes. Cache warming improves reuse but does not guarantee a candidate duration.
