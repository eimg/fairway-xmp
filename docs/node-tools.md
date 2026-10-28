# Node.js and native fnm

For task instructions, see [Install or switch Node.js](https://eimg.github.io/fairway-xmp/#node).

Starting with `0.1.0-rc.5`, installers for all three platforms include the pinned upstream **fnm** executable. Node itself is downloaded only when requested. macOS distribution applies the app’s normal code signing to fnm; its source and CLI behavior are unmodified. Use `fnm` directly; there is no `fairway node` or `fairway fnm` command.

| Platform | Detection and activation |
| --- | --- |
| macOS | Settings checks an interactive login Zsh environment, including nvm/Homebrew and inherited custom ZDOTDIR. Embedded terminals import exported tool settings, then use Fairway’s own prompt/history configuration. |
| Windows | Settings and embedded terminals refresh the system/user registry environment. Opting into fnm Node configures new embedded terminals and, with CLI tools enabled, new Command Prompt and PowerShell sessions. |
| Linux | An existing fnm keeps PATH priority; otherwise CLI integration or `fairway shell` exposes the bundled executable. Node activation is configured by the user’s shell. |

## Existing fnm installations

When Fairway finds an external fnm executable or an existing populated store without a Fairway ownership record, the Node.js card is **read-only**. It shows the selected Node, existing fnm path, storage path and installed versions, with copyable CLI commands under **Manage in your terminal**. Install, Use and Uninstall are unavailable; the backend also rejects those actions and enabling Fairway activation. Existing shell initialization and defaults stay under your control. This does not change how nvm/Homebrew Node setups can opt into a new Fairway-managed store.

CLI setup and embedded-terminal launchers skip the bundled fnm when an external executable is found, including when another Fairway launcher is earlier on PATH. Detection excludes Fairway's persistent and temporary launchers and symlinks to its bundled executable. Bundled fnm remains inside the app and can be invoked by its full path when needed.

If Fairway activation was enabled before another fnm appeared, the card offers **Turn off Fairway activation**. This explicitly removes Fairway's activation hooks for new sessions and preserves installed versions, global packages and the current shared default. It does not restore a historical default. Existing sessions retain their selection. Detection alone does not silently remove existing activation. On Windows the CLI copy needed by already-enabled startup hooks remains until that handover, so those hooks keep working.

A populated store left by an older development build without an ownership record is also read-only. Fairway does not infer ownership from an enabled marker or automatically adopt those versions. Empty stores are recorded in `fnm-store.json` in Fairway's application data only when a Fairway installation starts; release-list queries use a separate app-owned query directory. Detection and installed-version listing do not initialize or claim the Node store. Failed setup can be retried against the recorded store. Changing `FNM_DIR` to another populated, unrecorded store makes that store read-only.

## Desktop settings

The Node.js card sits above Command-line tools. It highlights the selected version, source manager and executable path. Other detected installations are listed separately; management is offered only when no external fnm is found and the store is empty or has a Fairway ownership record. A missing/broken fnm is reported separately from existing Node detection.

When management is available, Manage Node.js opens automatically when no active Node is found and detection succeeds. **Install & use** installs the selected version and enables it in new terminals. With an existing Node selection, **Install** preserves it; choose **Use** beside an installed version to activate it. The picker shows the latest stable patch per major from Node 20 onward, including non-LTS majors. It currently defaults to the newest major, which may be non-LTS. Exact older versions remain available with `fnm install VERSION`, and all installed versions stay visible.

Downloads show a progress bar and **Cancel download** while fnm downloads and installs Node. You can leave Settings and return to cancel the same installation. Cancellation stops the download process tree; retry from the picker. Once activation starts, setup finishes or rolls back before other operations resume.

**Use** selects the default and enables activation together. If shell setup fails, Fairway restores the previous default. A successful download is kept if activation fails, so you can repair the shell setup and retry Use.

Use affects new terminals. Existing terminals/development servers keep their selection until restarted; no project pins, directory-change hooks or automatic version switching are added. On macOS activation initializes new Bash/Zsh shells when CLI tools are enabled. Windows uses a reversible current-user Command Prompt AutoRun hook and managed blocks in the normal Windows PowerShell 5/PowerShell 7 profiles. Existing content is preserved. PowerShell execution policy is respected; `-NoProfile` and Command Prompt `/D` skip external hooks. Windows activation paths cannot contain `%`, `!`, quotes or line breaks.

**Use existing Node setup** removes Fairway’s activation. **Uninstall** removes the chosen fnm version and its global npm packages; removing an active default first disables Fairway’s activation. Stop terminals and servers using the version before uninstalling. Versions managed by nvm, Homebrew or other tools remain unchanged. Native fnm users should select an installed numeric default if they want it reflected as Fairway’s managed choice.

## Storage and shell use

On macOS Fairway reads an absolute `FNM_DIR` from the interactive login shell, including values set in personal shell files. Windows reads the refreshed system/user environment; Linux follows the invoking environment. Otherwise it follows fnm’s normal store: XDG data storage (normally `~/.local/share/fnm`) on macOS/Linux and roaming AppData `fnm` on Windows, retaining supported legacy `.fnm` stores. On Windows, a custom store set only in a PowerShell profile is not part of the registry environment; compare the displayed storage path with your shell before using the CLI. Node versions and global packages live outside the application and survive updates/removal. Removing CLI integration removes Fairway’s launchers/hooks and preserves the store.

For manual activation, Bash/Zsh use `eval "$(fnm env --shell bash)"` or `--shell zsh`. PowerShell uses `fnm env --shell powershell | Out-String | Invoke-Expression`; Fish uses `fnm env --shell fish | source`. Then use ordinary commands:

```sh
fnm install --lts
fnm use lts-latest
fnm list
fnm default VERSION
fnm uninstall VERSION
```

Fairway’s PHP services, desktop browser and ordinary CLI do not require a preinstalled Node runtime.

## Verification and troubleshooting

Refresh detection after changing managers or PATH. New embedded terminals read the configured tool environment; macOS export capture does not import personal aliases/functions, prompts or history hooks. Detection timeouts and fnm errors stay visible. If bundled fnm is missing and Fairway management is available, reinstall the complete Fairway package. In read-only mode, use the displayed existing fnm or bundled executable path and your normal shell initialization. If native fnm changed/deleted a Fairway-managed default outside Settings, select an installed version with Use to repair activation.

The noninteractive Mac/Windows fnm smoke examples use scratch stores/profiles, exercise downloads/defaults/uninstall and leave personal installations unchanged. The Linux fnm smoke checks the bundled executable and optional Node LTS installation with Node/npm/npx in an isolated store. Commands are in [Contributing](../CONTRIBUTING.md) and [Testing](testing.md). These checks do not replace native Windows execution or optional WebView review.
