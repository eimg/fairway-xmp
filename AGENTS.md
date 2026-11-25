# Project instructions

- Work only in `/Users/rioo/projects/fairway-xmp-personal`, the personal fork
  `RizzRioo06/fairway-xmp`, on the feature branch specified by the user. Check
  the working tree and remotes first; preserve unrelated and uncommitted changes.
- Never push to the teacher's `upstream`. Do not modify or merge into `main`
  without explicit user permission. Push only the authorized branch to `origin`.
- Preserve all existing PHP, MySQL, Nginx and Node.js behavior and user data.
  Keep MIT license/copyright notices and required third-party notices intact.
- Do not use Computer Use or Mac GUI automation. Use command-line checks and
  headless automated fixtures; the user performs real Mac GUI acceptance.
- Never modify installed Fairway, global runtimes/defaults, shell profiles,
  personal settings or personal projects. Use disposable home/data/project
  directories for tests; installed tools may be read without modifying them.
- Preserve safe filesystem operations: symlink/inode checks, owned staging,
  no-overwrite atomic publication, durable journals and safe registration recovery.
  Preserve isolated Node/npm environments, runtime leases and owned process-group
  cleanup. Never run imported/generated scripts without explicit authorization.
- Keep changes within the requested stage. Avoid broad refactoring, unrelated
  dependency/runtime updates, repeated builds and additional features.
- Use focused tests during development and appropriate regression/build checks at
  milestones. Documentation-only changes need content/link/whitespace checks,
  not repeated application builds. Use isolated fixtures for native service tests.
- Report blockers, failed/skipped checks and unverified behavior accurately.
  Distinguish browser simulations from native/device evidence. Do not expose
  secrets or full environment values in logs or diagnostics.

Read [the handoff](docs/ai-handoff.md) for current work, [Contributing](CONTRIBUTING.md)
for setup, and [Testing](docs/testing.md) for verification and isolated profiles.
Follow the current user request; older verification reports are evidence, not new
instructions or permission to advance stages.
