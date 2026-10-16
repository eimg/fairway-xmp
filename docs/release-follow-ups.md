# Follow-up work after PHP version selection

PR #1 was merged as `2114689`. The simpler Advanced settings dropdown, selected-runtime Linux recovery, Windows ZIP configuration, settings rollback, and regression tests are implemented. Repository Mac/Windows checks passed; native Windows/Linux validation exercised all six PHP versions, including Linux recovery and shutdown.

Beta 1 is published. The Windows PHP 8.0/8.1 provenance work below remains open and is disclosed with that release. It does not require changing the published beta 1 files.

## Legacy PHP scope for beta

Signing is deferred while beta recovery/release/support work is completed. PHP 8.0/8.1 remain legacy compatibility options, labeled in Advanced settings and CLI help; PHP 8.5 stays the default. No additional legacy runtime expansion or source-mapping research is scheduled in this pass. The provenance work below remains open and is disclosed in candidate metadata; labeling a runtime legacy does not verify its dependency sources.

The new [beta acceptance process](beta-acceptance.md) covers candidate identity, native lifecycle checks, restore verification, diagnostic reports, and final human review. A green browser run does not complete the remaining native checklist.

## Verify older Windows dependency sources

**Open before publishing a Windows installer with PHP 8.0/8.1.** Those official PHP ZIPs have no embedded SBOM identifying exact dependency build commits. `scripts/windows_legacy_sources.json` supplies 49 pinned source entries, and `runtime-sources.lock.json` records archive hashes. This verifies which source files are collected; it does not prove that their build recipes and patches match the distributed binaries.

Some mappings require particular attention:

| Binary dependency package | Recorded source tag |
| --- | --- |
| `libcurl-7.76.1-3-vs16-x64.zip` | `libcurl-7.76.1-2` |
| `libffi-3.3-4-vs16-x64.zip` | `libffi-3.3` |
| `libiconv-1.16-5-vs16-x64.zip` | `libiconv-1.16-1` |

Different build suffixes do not by themselves establish a source mismatch, but the mapping needs evidence. Passing runtime tests does not resolve that question.

- [ ] Match the dependency list against the notices in the exact checksum-pinned PHP 8.0/8.1 Windows ZIPs.
- [ ] Record upstream build evidence for the source revisions and package-specific recipes/patches, including the mappings above.
- [ ] Correct or extend the legacy source manifest and source lock where that evidence identifies missing inputs.
- [ ] Collect the corresponding-source archive and verify it includes those exact sources, recipes, patches, and notices.

If an exact mapping cannot be established, use a verifiable runtime input or defer those versions in the Windows release. Keep this item open until the evidence is recorded; version-matched tags alone are not completion.

## Finish the release owner's Mac visual test

- [ ] Confirm PHP selection is hidden in collapsed Advanced settings by default and uses one dropdown.
- [ ] Confirm Save changes the PHP card and selected web/CLI/Composer runtime, and the selection persists after Quit and reopen.
- [ ] Confirm the normal start/stop, close-window, menu-bar, and Quit experience remains straightforward.

The release owner tested the Mac beta 1 installer on 2026-10-15 and authorized publication. These three visual checks were not recorded individually. See [the PHP testing guide](testing-php-versions.md) for the commands.

## Publish the feature in a new release

- [x] Publish `v0.1.0-beta.1` from candidate run 38699450419, commit `f3cf84da6270038274590860db00c58eda2f1df6`. The installers include PHP version selection and report `0.1.0-beta.1`.
- [x] Upload those candidate files unchanged. `BUILD-INFO.json` records the commit, run, and hashes.
- [ ] Keep the Windows PHP 8.0/8.1 provenance item above open. Beta 1 discloses it and ships the matching Mac/Windows and Linux source archives beside the binaries.
- [x] Update README downloads and the Linux PHP-version instructions for beta 1.

See [Contributing](../CONTRIBUTING.md#publishing-a-release) for packaging and publication. Signing/notarization requirements for stable releases remain described there.
