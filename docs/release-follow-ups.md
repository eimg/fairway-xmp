# Follow-up work after PHP version selection

PR #1 was merged as `2114689`. The simpler Advanced settings dropdown, selected-runtime Linux recovery, Windows ZIP configuration, settings rollback, and regression tests are implemented. Repository Mac/Windows checks passed; native Windows/Linux validation exercised all six PHP versions, including Linux recovery and shutdown.

The remaining work is for the next release. The published alpha 3 installers contain PHP 8.5 only, so these follow-ups do not require changing that release.

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

A local Mac test installer was prepared from `2114689`; the release owner is testing it. Automated runtime and build checks passed, but visual-test results have not been recorded yet. See [the PHP testing guide](testing-php-versions.md) for the commands.

## Publish the feature in a new release

- [ ] Choose a new release label and use installers that actually include the merged PHP-selection behavior. The existing alpha 3 downloads cannot provide this feature.
- [ ] Confirm each candidate's source/runtime matches its validation results; retain the exact tested files and record original commits, hashes, and embedded versions when reusing them.
- [ ] Complete the Windows source verification above before shipping the older runtimes; include the matching Mac/Windows and Linux source archives beside the binaries.
- [ ] Update README downloads and the Linux PHP-version instructions to match the new release's capabilities.

See [Contributing](../CONTRIBUTING.md#publishing-a-release) for packaging and publication. Signing/notarization requirements for stable releases remain described there.
