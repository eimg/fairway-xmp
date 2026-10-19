# Standalone permission test builds

These are draft test apps, not a new release. They use your ordinary Fairway settings,
sites, databases and document root. Quit the installed app completely from its tray/menu
bar before opening a test copy. Quit the test copy the same way before returning to the
installed app; closing its window alone leaves Fairway running.

## Windows x64

The owner reported that the standalone Windows permission build works as expected.

Extract the entire ZIP into a permanent test folder such as `C:\FairwayPermissions`.
Open `Fairway XMP Permissions Test\fairway.exe`; keep its `runtime` folder beside it.
No Setup program is required. This uses the WebView2 and Visual C++ prerequisites already
present on a machine running the installed Fairway app. The EXE has no Authenticode
signature, as with RC 1. Its window is labeled **Permissions Test**.

Add a disposable site or rename a site address. The normal Windows administrator/UAC
prompt should appear without an empty PowerShell window behind it. Decline once and
retry through **Set up addresses**. The site is saved before address setup; do not add
it twice. Start Nginx and confirm the address works.

For HTTPS, enable it on a site and approve Windows' certificate trust confirmation.
There should be no empty console behind that confirmation. Existing trust may skip the
confirmation; to test fresh trust, turn HTTPS off for every site (the last switch also removes
certificate trust), then enable it again. Also test cancellation/retry and trust removal. The change
suppresses console windows, while retaining Windows security confirmations and errors.

When done, quit from the tray and reopen the installed app. Do not enable/disable CLI
integration merely to test permission prompts; that would change your user PATH.

## Mac Apple Silicon

Unzip the archive and open **Fairway XMP Permissions Prototype.app** directly; do not
run an installer or overwrite the installed app. The test app is Developer ID signed
and must be notarized/stapled before hosts testing. Its window says **Native Permissions
Prototype**. The archive's build metadata records its notarization status.

The earlier test build skipped registration when macOS reported a fresh helper as
"not found." The corrected build registers that state as well as "not registered";
its signed-app CI checks registration and cleanup without approval or hosts writes.

Adding or renaming an address registers the native hosts helper when needed. macOS
requires one-time administrator approval in **System Settings → General → Login Items
& Extensions**. Fairway opens that page. Approve the Fairway XMP / Ei Maung helper, then
return and click **Set up addresses**. The site is already saved. macOS chooses Touch ID
or credentials for this approval. Later address changes should reuse the approved
helper without another password prompt. Test add, rename, remove and cancelled approval.
The helper is machine-wide and modifies only Fairway's loopback `.test` entries.

The native HTTPS implementation is the one already reported to work with one Touch ID
prompt. Existing CA trust is reused. Test it independently from hosts approval.

### Issue #9: authorization focus

The focus test build relinquishes Fairway's active state before native keychain/trust
authorization and suppresses Fairway's own window-focus requests until that call ends.
The same handoff now covers removal, legacy hosts authorization and updater installation.
It cooperatively yields to System Settings before opening Login Items, and Mac
errors stay in Fairway's panel without raising the app over Settings. This adds no
authentication prompt and preserves the one-time helper approval model. Actual system
dialog focus still needs device verification; unit tests cannot establish it.

The owner confirmed that enabling HTTPS focuses the Touch ID prompt in build
`88fa366`, but removing trust did not. This follow-up covers both directions.

Test HTTPS separately: turn HTTPS off for every site. The last switch now also removes
certificate trust, so there is no second cleanup click. If cleanup is cancelled, HTTPS
stays off and **Remove certificate trust** appears for retry. Test that retry too.
Enable HTTPS again on an existing site. When the authorization prompt appears,
try Touch ID immediately, without clicking the prompt first. Cancel once, retry, and
also check the password fallback for both enabling and removal. With two HTTPS sites,
turn one off: there must be no trust-removal prompt, and the other site must still work. Confirm Fairway does not jump in front while the
prompt is open, and that returning to Fairway works after either outcome.

For fresh helper approval, quit the previous prototype and unregister its helper
using the command below **before moving its app bundle**. Open the new build and add
or rename an address. Confirm Settings comes forward and its approval prompt accepts
Touch ID without first clicking it. Return to Fairway, retry **Set up addresses**,
then add another site: Settings must not reopen for the approved helper. Record the
macOS version and results for HTTPS and helper approval separately.

Before moving/deleting the test app or returning to RC 1, quit Fairway and unregister
the helper. In Terminal, use the actual path to the test bundle:

```sh
'/path/to/Fairway XMP Permissions Prototype.app/Contents/MacOS/fairway' --remove-native-hosts-helper
```

This keeps your saved sites, hosts entries and CA trust. Removing the last site address
also unregisters the helper. Then reopen the installed app normally.

## Shared UI checks

The Add/Edit site dialog keeps short labels and one approval notice. Question-mark
buttons contain the longer domain, folder and routing explanations. Escape closes a
help tip first, then the dialog on a second press. Validation errors remain visible.
Existing `index.php` files are preserved; the smaller welcome page appears only when
Fairway creates a missing default index.

Record the OS, embedded commit from `--build-info`, prompt/console behavior and whether
cancellation followed by retry works. Existing RC 1 remains the installed fallback.
