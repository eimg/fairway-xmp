# macOS authorization assessment — 2026-10-19

Status: assessment plus opt-in native HTTPS-trust and hosts-helper prototypes, prepared after RC 1 at the owner's request for a local Mac comparison build. Standard builds keep the existing authorization behavior. The owner will decide separately whether the proposed work warrants another RC.

## Findings in the current source

| Operation | Current implementation | Meaning for the prompts |
| --- | --- | --- |
| Add, rename or remove a site address | `hosts::ensure` compares the desired hosts block, then launches the existing short-lived Fairway helper through AppleScript `with administrator privileges` when a change is necessary. | This path requests administrator authorization. There is no Touch ID setting in Fairway's current implementation. Unchanged addresses already skip elevation. |
| First HTTPS trust setup | `tls::ensure_trust` generates certificates without elevation, checks existing trust, then invokes `security add-trusted-cert` with SSL trust in the current user's login keychain. | This is user certificate trust, not administrator access to the system keychain. Certificate insertion/keychain access and trust-setting authorization can involve separate system interactions. |
| Subsequent HTTPS sites | Existing verified CA trust is reused. | Per-site certificate generation does not itself require another trust approval. Removing the last HTTPS site removes trust; setting up HTTPS again can therefore require approval again. |

Sources: [hosts implementation](../src-tauri/src/hosts.rs), [TLS implementation](../src-tauri/src/tls.rs), and [desktop site commands](../src-tauri/src/desktop.rs). The user's report describes a password prompt followed by a Touch ID-capable prompt during HTTPS setup. That is consistent with separate keychain/trust interactions; the exact responsibility of each dialog is **not established** without its wording and a controlled reproduction. No fresh trust-store mutation was performed for this assessment.

Read-only probes on the available Mac (reported by `sw_vers` as macOS 27.2, build 26B5112a) found Touch ID available through `LAContext.canEvaluatePolicy`, and distinct authorization rules for `system.privilege.admin` and `com.apple.trust-settings.user`. The latter delegates to `entitled-session-owner-or-authenticate-session-owner`; the administrator right is a separate user authorization. These observations support treating the operations separately. They do not establish which authentication methods either actual operation will offer. The initial sandboxed probes could not contact the system services; the successful probes ran outside that restricted context. Neither probe evaluated an authentication policy, displayed a prompt, or modified authorization rules. This device snapshot does not replace the owner's separately reported RC test OS details.

## What the native APIs can and cannot improve

Apple's [Authorization Services](https://developer.apple.com/documentation/security/authorization-services?language=objc) delegates authentication to the system security agent and allows authentication mechanisms to evolve. It does not promise that every requested right can use Touch ID.

[Local Authentication](https://developer.apple.com/documentation/localauthentication/lapolicy/deviceownerauthentication) can offer Touch ID, Apple Watch and password fallback on supported Macs. A successful local authentication result does not itself give an ordinary process permission to modify `/etc/hosts`. Adding an independent biometric dialog before the existing command could add friction without replacing its authorization.

Apple documents a separate authentication interaction for [per-user certificate trust changes](https://developer.apple.com/documentation/security/sectrustsettingssettrustsettings%28_%3A_%3A_%3A%29). Replacing the `security` subprocess with Security framework calls may improve integration and control of the sequence, but fewer prompts or Touch ID availability must be demonstrated on the supported OS versions. User trust must remain user trust; moving the CA to system-wide trust would unnecessarily broaden its scope.

## Recommended next experiment

Investigate **native per-user HTTPS trust setup first**, independently of hosts elevation. The opt-in `native-macos-trust` Cargo feature now adds the same public CA certificate to the login keychain with `SecItemAdd`, then applies SSL-server-only trust in the user domain with `SecTrustSettingsSetTrustSettings`. It decodes the actual PEM used by verification rather than relying on a potentially stale DER sidecar. Existing trust verification and compatible certificate/trust removal are retained. It adds no preliminary Local Authentication dialog, stored-password keychain unlock, administrator helper or authorization-policy changes. A duplicate public certificate left after cancellation is accepted for retry; other insertion errors stop before requesting trust. The trusted marker is written only after the existing verification succeeds. Cancellation and other errors retain the failed operation's context.

The four optional Mac dependencies already existed in Cargo.lock; their versions and checksums are unchanged. Ordinary builds leave the feature off, and Windows/Linux keep their existing trust implementation. The local comparison build enables it explicitly and labels its window **Fairway XMP — Native HTTPS Prototype**. It uses the ordinary Fairway profile and therefore must be run after quitting the installed app; it does not replace the installed RC 1 bundle. A Developer ID signature identifies the local app, but this is a test build rather than a notarized release candidate.

For a meaningful prompt comparison, existing CA trust must first be absent. In the app, turn HTTPS off for all sites, choose **Remove certificate trust**, then enable HTTPS for one existing site. Merely enabling another site while the CA is already trusted intentionally skips setup. The owner tested this build and reported one seamless Touch ID prompt on this Mac. This is an owner-reported result; other supported macOS versions, locked keychains and non-biometric accounts remain unverified. Cancel a setup and retry, enable a second HTTPS site, then remove trust when unused. The separate hosts experiment below replaces the administrator-password subprocess only when explicitly enabled. These steps affect the normal profile's local CA trust; no automatic trust removal or setup is run during build/verification.

Use a disposable logged-in test account and a dedicated test CA. Record the exact dialog text, requesting process, prompt count and available authentication methods for the current implementation and prototype. Cover an unlocked and locked login keychain, clean and already-trusted CA, cancellation at each prompt, Touch ID unavailable/disabled, a standard user account, a second HTTPS site, trust removal and re-creation. Check the oldest supported macOS release as well as the owner's macOS 27 beta; the beta alone cannot establish shipping behavior. Remove only the dedicated test CA afterward.

Proceed only if the prototype gives a measured improvement while preserving trust scope, cancellation/retry behavior, verification and removal. The expected cost is a small native integration plus moderate device verification; prompt reduction remains an open result, not an implementation promise.

## Native hosts prototype

At the owner's request following the successful HTTPS comparison, `native-macos-hosts` adds a separate experimental route using [SMAppService](https://developer.apple.com/documentation/servicemanagement/smappservice). The signed app bundles an on-demand LaunchDaemon and registers it only when addresses need changing. macOS requires one-time administrator approval in System Settings → General → Login Items & Extensions. Fairway opens that page and asks the user to retry the address change after approval. Touch ID availability for this approval belongs to macOS; this is not a promise of the same one-prompt experience as HTTPS. Subsequent changes reuse the approved service. The signed standalone build now checks fresh registration and unregisters it immediately, without administrator approval, XPC calls or hosts writes. Ordinary PR checks remain non-mutating.

The daemon exposes one NSXPC method accepting at most 64 KiB of JSON containing up to 1024 validated single-label site names. It can only synchronize loopback `.test` entries inside the existing Fairway hosts block. It accepts no path, command, IP address, certificate or executable from callers. Its Rust writer reuses the existing conflict checks, preservation of unrelated entries and atomic replacement of `/etc/hosts`; writes are serialized with at most four pending requests. Malformed requests are rejected before writing. This is a machine-wide capability: another local account able to run the same signed helper client can request these narrowly scoped Fairway address changes after approval. It does not implement per-account ownership of the shared hosts block, which the current application also lacks.

The identical signed helper binary implements client and server modes. Both sides use macOS 13+ native XPC code-signing requirements, pinned to its exact identifier, the selected Apple Developer team and Developer ID certificate class; the privileged Mach namespace selects the system service. The server requires root and the client requires a regular user. No deprecated `AuthorizationExecuteWithPrivileges`, arbitrary root subprocess, password storage, policy modification or preliminary biometric prompt is added. The simpler `authopen` approach was rejected because it cannot retain atomic file replacement; NSWorkspace's privileged file operations require a restricted entitlement.

`scripts/build_macos_hosts_helper.py --team-id TEAMID --output DIRECTORY` builds the minimal Rust static writer plus Swift executable and prepares `Library/HelperTools` and `Library/LaunchDaemons`. Copy `Library` into the final app's `Contents` before running the normal nested Developer ID signing script with the matching team. Apple requires apps containing these LaunchDaemons to be notarized: submit the complete signed app, staple its accepted ticket, and verify Gatekeeper before testing registration. Apple recommends `/Applications` for daemons needed before login; this prototype is for logged-in testing and must be unregistered before its bundle is moved or deleted. The helper build must precede the desktop build so each gets its own Cargo feature set. Default release packaging does not include or enable this experimental capability. Mac PR CI compiles both native features, runs their non-mutating tests and builds the helper with a placeholder identity; it does not register or approve a daemon.

Removing the final managed address also unregisters the helper, including when the hosts block was already empty. Native-feature uninstall uses that same path. For a test build with sites retained, quit Fairway and run the prototype executable with `--remove-native-hosts-helper` before deleting/moving the bundle or returning to RC 1; this unregisters the daemon without changing saved sites or hosts entries. The existing save flow persists the site before hosts synchronization. Pending or declined approval therefore leaves the saved site in the list with **Set up addresses** available for retry; validation errors still remain in the form. The owner confirmed native background-helper approval through Touch ID and subsequent site additions without another prompt. Cancellation/retry, standard-user behavior, final-site cleanup and approved-helper migration still need separate acceptance. Ship/update migration of an approved helper needs separate acceptance before enabling the feature in ordinary builds.

The owner reported that Windows permission handling works as expected. On the first Mac hosts test, the bundle was complete but SMAppService returned NotFound with no registration record; the previous code only registered NotRegistered, leaving Set up addresses stuck. The corrected path registers either state after validating the payload. The standalone signed-app job exercises registration and cleanup so bundle checks alone cannot count as registration acceptance. That check also exposed the documented EPERM response while administrator approval is pending; the app now checks the resulting service status even when registration returns an error. See [Apple’s walkthrough](https://developer.apple.com/forums/thread/802443). The owner subsequently confirmed the corrected build works: macOS asks to allow Fairway’s helper to run in the background; approval accepts Touch ID, and additional sites do not prompt again. Use this exact background-helper wording in the product and test instructions.

The Add/Edit form now keeps one short approval notice and a neutral localhost note with folder-access details in its tooltip. Domain, document-root and routing explanations are available through help buttons that work with mouse, keyboard and touch; Escape dismisses a help tip before closing the dialog.

## Issue #9 focus handoff implementation — 2026-10-19

The opt-in native desktop build now performs AppKit handoffs on the main thread.
Before login-keychain access and SSL trust setup it calls `NSApplication.deactivate`
once, without identifying or activating a security-agent process, then holds a scoped
guard that suppresses Fairway's explicit focus requests until the native call returns.
The blocking Security calls remain on the worker thread. The guard releases on success,
cancellation and failure without automatically reactivating Fairway.

Before `SMAppService.openSystemSettingsLoginItems`, Fairway yields activation to
`com.apple.systempreferences` on the same main-thread turn. This follows Apple's
[cooperative activation guidance](https://developer.apple.com/documentation/appkit/passing-control-from-one-app-to-another-with-cooperative-activation).
Unlike Settings, the Security API exposes no destination app to yield to; the one-shot
deactivation is an experiment requiring device acceptance. Native-build error reporting
no longer raises Fairway after opening Settings. Existing helper registration, reuse,
certificate scope and trust verification are unchanged. No additional authentication,
Accessibility access, simulated input or timed activation retry is introduced.

Non-mutating regression tests cover scoped focus suppression and release after errors;
they cannot prove Touch ID focus. The implementation host reports macOS 27.2
(26B5112a); no fresh authentication or trust/hosts mutation was performed by the agent.
The owner's actual prompt-focus, cancellation, password-fallback and helper-reuse
results for this build remain pending. See [test steps](permission-test-builds.md#issue-9-authorization-focus).

## Follow-up: removal focus and authorization audit — 2026-10-19

The owner tested signed/notarized build `88fa366` and confirmed that HTTPS-enable
Touch ID authorization focuses automatically. They reported certificate-removal
Touch ID authorization still needed a click. This isolated the missing removal path:
`security delete-certificate -t` was outside the scoped handoff.

The shared guard now covers both native and compatibility HTTPS installation, all
certificate-removal callers, legacy hosts elevation, and the updater's potentially
privileged installation. Read-only trust verification, ordinary service controls,
CLI/PATH integration and approved-helper XPC requests do not request elevation and
retain normal focus. Settings retains the cooperative handoff for initial helper
approval. Installer-owned authorization runs in Installer, and the standalone
uninstaller requires Fairway to quit first; its dialogs activate themselves, and
Finder is activated before its own deletion approval. Terminal-owned sudo prompts
remain terminal interactions. No extra authorization or security-agent targeting
was introduced. Folder-picker/TCC permission UI remains system-owned; no blanket
focus manipulation is applied to ordinary file access.

In the desktop app, turning off the last HTTPS site now applies the site/config
change and removes unused shared CA trust in one operation. Other enabled HTTPS
sites retain trust. If cleanup is cancelled or fails, HTTPS stays off and the panel
reports that cleanup did not finish; **Remove certificate trust** remains available
as a retry. This avoids silently re-enabling HTTPS or requiring a second cleanup
click in the successful flow. The existing uninstall and remove-site cleanup paths
use the same guarded trust-removal function.

Device results for removal, the broader authorization paths and cancellation remain
pending the follow-up build. No trust removal, elevation, updater installation or
uninstall was triggered by the agent during this audit.

## Release recommendation

Keep RC 1 and its published artifacts unchanged. The accompanying welcome-page proposal is independent visual/content polish. The native trust feature remains an opt-in test proposal and has a successful owner-reported HTTPS comparison. The owner has accepted the hosts background-approval and subsequent-add flow for the next release; release packaging/default-feature integration and the remaining acceptance cases still need completion. Any welcome-page or authorization implementation chosen for another RC requires a fresh candidate build and acceptance; existing RC 1 hashes do not cover it.


## Saved checkpoint — 2026-10-19

The owner intends to include the current welcome-page, concise site form and native permission changes in the next release. Windows permission handling works as expected; Mac HTTPS accepts one Touch ID prompt; Mac hosts setup accepts one background-helper approval with Touch ID and reuses it for additional sites. These are owner-reported results, not blanket acceptance of every account/OS/migration case.

The corrected standalone Mac source is `0174c080dd808a0161ac59077a1501cb7508249a`. [Standalone CI](https://github.com/eimg/fairway-xmp/actions/runs/39202318123) and [all branch checks](https://github.com/eimg/fairway-xmp/actions/runs/39202320388) passed. The local artifact is `artifacts/permission-tests/0174c08/macos/Fairway XMP Permissions Prototype.app`; checksum, clean embedded commit, signatures, notarization and fresh registration/cleanup (3 → 2) also passed locally. Draft PR #8 remains unmerged, and published RC 1/feeds are unchanged. Ordinary builds still leave both native Mac features disabled; enabling and packaging them for the next candidate is outstanding.

Next small investigation: the owner reports that the Touch ID dialog appears without focus while Fairway remains active, requiring a click before authentication. Check which approval flow exhibits it, avoid Fairway reclaiming focus during approval, and explicitly hand activation to System Settings when opening background approval. AppKit offers cooperative activation/yield APIs, so this appears likely to need a small Mac-only change rather than a permission redesign; the system-owned authentication window still requires device verification. Do not promise focus control of arbitrary security dialogs, add extra authentication prompts, or use Accessibility permissions, simulated clicks or timed focus loops. No focus fix has been implemented yet. Resume with this issue, then confirm remaining device cases and next-candidate packaging before deciding on another RC.
