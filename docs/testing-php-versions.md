# Test PHP version switching

Use an isolated home and data folder so development runs do not touch an installed Fairway XMP environment. PHP selection is global for this app instance: websites, the PHP launcher, and Composer use the same selected version.

## Automated checks on macOS

From the repository root on an Apple Silicon Mac with Xcode Command Line Tools, Node.js 24, Python 3.12+, and Rust 1.90+:

```sh
npm ci
npm run build
python3 -m unittest discover -s tests -v
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
python3 scripts/prepare_runtime.py
python3 scripts/prepare_runtime.py --verify-only
SMOKE_DIR=$(mktemp -d /private/tmp/fairway-smoke.XXXXXX)
cargo run --manifest-path src-tauri/Cargo.toml --example smoke --no-default-features --locked -- "$SMOKE_DIR"
```

The last command starts isolated services and checks PHP and Composer, a PHP → MySQL page, phpMyAdmin, backups, shutdown, and CLI plus web requests under every bundled PHP version. It prints `PASS` when those checks succeed.

## Test the macOS desktop panel

1. In a terminal at the repository root, create isolated folders and start the development app:

   ```sh
   export FAIRWAY_DEV_HOME="$PWD/.runtime-work/php-test-home"
   export FAIRWAY_DEV_DATA="$PWD/.runtime-work/php-test-data"
   export ZDOTDIR="$FAIRWAY_DEV_HOME"
   mkdir -p "$FAIRWAY_DEV_HOME" "$FAIRWAY_DEV_DATA"
   npm exec --yes --package @tauri-apps/cli@2.12.2 -- tauri dev
   ```

2. In **Settings → Service Settings → Advanced settings**, confirm **PHP 8.5** is selected. Click **Start all** and wait for **All services ready**.
3. In another terminal at the repository root, create a test page:

   ```sh
   printf '<?php echo PHP_VERSION;\n' > .runtime-work/php-test-home/fairway/www/version.php
   curl http://127.0.0.1:8800/version.php
   .runtime-work/php-test-data/bin/php --version
   .runtime-work/php-test-data/bin/composer --version
   ```

   The web response and PHP launcher should report `8.5.11`.
4. Click **Stop all**, select each PHP version from **8.0** through **8.4** under **Settings → Service Settings → Advanced settings**, click **Save**, then **Start all**. Repeat the `curl`, PHP, and Composer commands for each one. The web response and PHP launcher should report the selected version from `runtime-manifest.json`; Composer should still start. Stop all services before each switch.
5. Quit through the menu bar icon. Start the app again with the same environment variables and confirm the last selected version is still selected. Switch back to 8.5 if desired.

## Windows

On Windows 10 22H2 or Windows 11 x64, run the automated commands above in PowerShell, replacing `python3` with `python` and the smoke directory with a new local folder. Stage the Windows runtime with `python scripts/prepare_runtime.py` before running the smoke example.

For an isolated desktop test, set both debug paths before `tauri dev`:

```powershell
$env:FAIRWAY_DEV_HOME = Join-Path $PWD '.runtime-work/php-test-home'
$env:FAIRWAY_DEV_DATA = Join-Path $PWD '.runtime-work/php-test-data'
npm exec --yes --package @tauri-apps/cli@2.12.2 -- tauri dev
```

Use the same panel steps. Create the test page, open it in a browser, and check the launcher:

```powershell
Set-Content -Path (Join-Path $env:FAIRWAY_DEV_HOME 'fairway/www/version.php') -Value '<?php echo PHP_VERSION;'
Start-Process 'http://127.0.0.1:8800/version.php'
& (Join-Path $env:FAIRWAY_DEV_DATA 'bin/php.cmd') --version
```

Beta 1 includes PHP 8.0 through 8.5. PHP 8.5 is the default, and PHP 8.0 and 8.1 are legacy compatibility options.

## Linux terminal edition

On an Ubuntu 22.04 x86_64 builder, stage the locked runtime with `python3 scripts/prepare_runtime.py --lockfile runtime-linux-downloads.lock.json`, then run the smoke command above with an isolated directory. To test an installed development build, stop services and switch versions:

```sh
fairway stop all
fairway php-version 8.0
fairway php --version
fairway composer --version
fairway start all
```

Check that a page in the document root reports PHP 8.0.30. Repeat for PHP 8.1 through 8.5, stopping services before each switch. The Linux installer workflow builds and smoke-tests this runtime when dispatched manually.
