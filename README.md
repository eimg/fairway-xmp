# Fairway XMP

A PHP development environment for Windows, Apple Silicon Macs and Linux x86_64. Run your web server and database from one control panel, write code in your preferred editor, and manage your data in phpMyAdmin.

![Fairway XMP control panel with PHP, Nginx, MySQL, and phpMyAdmin running](docs/images/control-panel.png)

## Downloads

Current release: **[v0.1.0-beta.1](https://github.com/eimg/fairway-xmp/releases/tag/v0.1.0-beta.1)**.

| Computer | Requirements | Download |
| --- | --- | --- |
| Windows | Windows 10 22H2 or Windows 11, x64 | [Windows installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-windows-x64-setup.exe) |
| Mac | macOS 15 or later, Apple Silicon (M-series) | [Mac installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-macos-arm64-unsigned.pkg) |
| Linux | x86_64, glibc 2.35 or newer | [Terminal installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-linux-x86_64.run) · [Archive](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.1/fairway-xmp-0.1.0-beta.1-linux-x86_64.tar.gz) |

Beta 1 installs report `0.1.0-beta.1`. Download the installer for your computer and follow its prompts. The installers are unsigned, so Windows may show an unknown-publisher warning. If macOS blocks the installer or app, follow [Apple’s instructions for opening an app from an unidentified developer](https://support.apple.com/en-us/102445).

## Linux terminal edition

Download the Linux `.run` installer or `.tar.gz` archive from the table above. Linux requires x86_64 and glibc 2.35 or newer; Alpine/musl and ARM are not supported. No desktop, root access, distro packages, Docker or systemd service is required.

```sh
sh fairway-xmp-0.1.0-beta.1-linux-x86_64.run
# Reopen your shell after installation:
fairway                      # terminal dashboard
fairway start                # start all services
fairway status
fairway stop nginx           # also stops its PHP-FPM worker
fairway php --version
fairway composer --version
fairway mysql fairway         # connect with the local teaching account
fairway backup               # requires running MySQL
fairway shutdown             # stop services and the supervisor
```

In the dashboard, use ↑/↓ to select Nginx or MySQL, `s` to start, `x` to stop, `r` to restart, `a` to start all, `z` to stop all, `l` for logs, `f` for PHP logs, `p` for ports and `b` for a database backup. **`q` closes the dashboard and leaves services running; `Q` shuts everything down.** Run `fairway --help` for CLI commands. phpMyAdmin remains a browser interface at the displayed URL.

The installer defaults to `~/.local/share/fairway-xmp`; use `--prefix /absolute/path` for another dedicated directory. It enables Fairway, PHP and Composer through managed Bash/Zsh profile blocks. `fairway remove-cli` removes that integration. `fairway shell` opens a shell with bundled PHP/Composer available without editing profiles. Fish users can invoke the displayed absolute Fairway path or add it to their shell’s PATH themselves.

Alternatively, extract the `.tar.gz` and run `./fairway-xmp/bin/fairway`; `./fairway-xmp/install.sh` installs that extracted bundle. Programs use libraries bundled with their component. Projects remain in `~/fairway/www`; data and backups use `${XDG_DATA_HOME:-~/.local/share}/dev.fairway.environment`.

Re-running the installer backs up an existing student database and stops services before switching versions. `fairway uninstall` removes the installed program and managed shell integration while preserving projects, databases and backups. The release’s Linux source archive includes corresponding runtime sources and distribution patches. This terminal edition uses manual installer updates.

## What’s included

- **PHP 8.5.11** with common extensions for PHP projects and Laravel.
- **Composer 2.10.3** for installing PHP packages.
- **Nginx 1.30.5** for serving your projects.
- **MySQL 9.7.2 LTS** for local databases.
- **phpMyAdmin 5.2.3** for managing tables and data in your browser.

The control panel provides service controls, port settings, logs, and database backups.

## Your first PHP project

1. Open Fairway XMP and click **Start all**. Wait for **All services ready**.
2. Click **Open folder** to open your project folder, `fairway/www` in your home folder.
3. Create a folder named `hello` and add an `index.php` file:

   ```php
   <?php
   echo 'Hello from Fairway XMP!';
   ```

4. Open [http://127.0.0.1:8800/hello/](http://127.0.0.1:8800/hello/) in your browser.

### Project addresses

You don't need a project to use Fairway: files in `~/fairway/www` always open at `http://127.0.0.1:8800/`. Add a project when an app needs its own address, such as `http://shop.test:8800/`.

Choose **Add project**, then enter a name, a `.test` domain (suggested from the name), and the **Web root**: the folder Fairway serves. The folder can be anywhere on your computer. For Laravel, choose the project's `public` folder; files outside the web root are not served. Turn on **Route missing paths to index.php** for Laravel and other apps with a front controller. Saving updates Nginx and adds the domain to Fairway's marked section of the system hosts file; macOS asks for an administrator password and Windows asks for administrator permission. If you cancel that request, the project stays saved and **Update hosts file** retries. Edit or remove a project at any time; removing it keeps its folder and files. If a web root folder is moved or deleted, the project shows as unavailable until you edit or remove it. Folders inside `~/fairway/www` are also served at `127.0.0.1:8800`, so keep apps with private files (such as `.env`) outside it. The website port is still required in `.test` addresses.

Each project has an optional **HTTPS** switch. The first time you turn it on, Fairway explains that it creates a local certificate authority and asks your operating system to trust it: on macOS it is added to your login keychain (approve with your password), and on Windows to your user certificate store (confirm the security warning). Neither needs an administrator account. HTTPS projects open at `https://domain.test:8443/`; the HTTP address keeps working. When no project uses HTTPS, **Remove certificate trust** removes it. Uninstalling Fairway removes its hosts entries and certificate trust. The certificate authority and private key stay in Fairway's application data and must not be shared or copied to another device. For Laravel, set `APP_URL` to the HTTPS address when you want Laravel to generate secure links.

On Linux, use `fairway project add shop ~/code/shop/public --name "My Shop" --index`, `fairway project remove shop`, `fairway project https shop on|off`, and `fairway project hosts` to retry a hosts-file update. `fairway status` lists projects. Hosts-file and certificate changes ask for your sudo password. Browser trust stores can differ on Linux; if a browser still warns, check its certificate settings rather than bypassing the warning.

Closing or minimizing the window keeps Fairway XMP and its services running. Use its macOS menu bar or Windows system tray icon to reopen the panel, start/stop all services or individual Nginx/MySQL services, open phpMyAdmin, or **Quit**. Nginx automatically manages its PHP worker; PHP CLI works without starting services. Quit gracefully stops all services before exiting.

## PHP and Composer in your terminal

After installation, reopen your terminal and editor so they can find PHP and Composer:

```sh
php --version
composer --version
```

Manage this integration under **Settings → Command-line tools**. You can also use **Open terminal** in the control panel.

The beta 2 candidate adds an embedded **Terminal** page on macOS and Windows. Press **Ctrl+backtick** to open or focus it. PHP and Composer are ready even when persistent command-line integration is disabled. Use **New terminal** to keep a second shell beside a running development server, and choose a project under **Start in** to open that new shell directly in its folder. Both tabs stay open while switching pages or hiding the window; quitting Fairway ends them. **Restart**, **End session**, and **Close tab** affect only the selected terminal. Find, text-size controls, and an expanded view help with longer output.

On macOS, Fairway remembers roughly 1,000 commands privately on your device, separately from your usual shell history. Start a command with a space to omit it, or turn off **Remember commands in new sessions** before creating or restarting a shell. Windows keeps its normal Command Prompt history within each session. macOS and Windows also add a **Data browser** beside phpMyAdmin, with `fairway db` commands for SQL import/export. Neither feature is in the published beta 1 installers.

Beta 1 includes PHP 8.0 through 8.5. Switch under **Settings → Service Settings → Advanced settings** after stopping all services. Select the version and click **Save**. The selection controls websites, terminal PHP, and Composer, and is saved for the next launch. On Linux, use `fairway php-version 8.0` through `fairway php-version 8.5` after `fairway stop all`. See the [step by step test guide](docs/testing-php-versions.md). PHP 8.5 is the default. PHP 8.0 and 8.1 are end of life; use newer versions for internet-facing projects.

For projects with their own development server, run the usual command, such as Laravel’s `php artisan serve`.

## Your database

Click **Open phpMyAdmin**, or visit [http://127.0.0.1:8801/](http://127.0.0.1:8801/). Sign in with username **fairway** and password **fairway**.

Use these connection settings in your PHP projects:

| Setting | Value |
| --- | --- |
| Host | `127.0.0.1` |
| Port | `13306` |
| Database | `fairway` |
| Username | `fairway` |
| Password | `fairway` |

You can create additional databases in phpMyAdmin. Services accept connections from your computer only.

To save a SQL backup, click **Back up databases**. Find saved backups under **Settings → Open backups**, and import them through phpMyAdmin when needed.

For complete database recovery, including routines and triggers, follow the [backup and restore guide](docs/backup-and-restore.md). Support reports are available under **Settings → Help and diagnostics** or with `fairway diagnostics` on Linux.

## Ports

| Service | Default port |
| --- | --- |
| Website | `8800` |
| phpMyAdmin | `8801` |
| MySQL | `13306` |
| PHP’s connection to Nginx | `19000` |

If a port is already in use, Fairway XMP shows the conflict. Stop all services, change the port in **Settings**, and save. If you change the MySQL port, use the new port in your project’s database settings.

## Updates and removal

Open **Updates → View downloads** to get a newer installer. On a Mac, the installer quits a running Fairway XMP and stops its services before replacing the app. On Windows, quit Fairway XMP from the system tray before installing. Your projects and database data are kept.

To uninstall:

- **Windows:** open **Settings → Apps**, select **Fairway XMP**, and choose **Uninstall**.
- **Mac:** quit Fairway XMP, then open **Applications** in Finder. Right-click **Fairway XMP → Show Package Contents**, open **Contents → Resources**, and double-click **Uninstall Fairway XMP.command**.

The uninstaller removes the app and its command-line integration. Your projects and database data are kept unless you choose to delete them.

## Help and license

For problems or suggestions, [open an issue](https://github.com/eimg/fairway-xmp/issues). Include your operating system, what you tried, and any error shown in Fairway XMP.

Fairway XMP’s application code uses the [MIT license](LICENSE). Included tools have [their own licenses](THIRD_PARTY_NOTICES.md). Release checksums and bundled-tool sources are available on the [release page](https://github.com/eimg/fairway-xmp/releases/tag/v0.1.0-beta.1). For contributing to the app, see [Contributing](CONTRIBUTING.md).
