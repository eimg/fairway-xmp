# PHP settings

For task instructions, see [Change PHP limits or timezone](https://eimg.github.io/fairway-xmp/#settings) and [Switch the active PHP version](https://eimg.github.io/fairway-xmp/#php).

Open **Settings → PHP settings** in the desktop app. The card starts with the active PHP version, normally 8.5. **Choose another PHP version** reveals the other bundled branches. Editing a branch does not switch the active interpreter; switch under **Service Settings → Advanced settings** when needed.

| Setting | Default | Meaning |
| --- | --- | --- |
| Memory limit | 256 MiB | Per PHP process; `-1` means unlimited. Supported positive values start at 16 MiB. |
| Execution time | 120 seconds | PHP script execution limit; `0` means unlimited. Nginx waits 30 seconds longer between FastCGI response reads. With `0`, PHP has no execution limit and Nginx allows 24 hours of response inactivity. CLI PHP normally starts with no time limit. |
| Upload size per file | 64 MiB | PHP's per-file limit. PHP POST and Nginx request limits are 8 MiB larger for multipart headers and other form fields. Several files together must fit within that request limit. |
| Timezone | UTC | Choose an identifier supported by that PHP version, such as `Asia/Yangon`. |
| Display errors | On | Show PHP errors in website responses. Error logging stays enabled. |
| Session upload progress | On | Controls `session.upload_progress.enabled`. Session-based tracking requires the application's normal PHP upload-progress setup. Turning this off leaves normal file uploads and browser-side progress available. |

PHP execution time and Nginx response inactivity measure different things. On Unix, time spent waiting on external work may not count toward PHP’s execution limit; the aligned Nginx timeout remains an idle-response limit rather than a guaranteed total request duration.

Stop Nginx and PHP before saving settings for the active version; MySQL and Mail can keep running. New PHP/Composer commands and the next web-server start use the saved values. Already-running PHP commands and project development servers keep their current configuration until restarted. Settings for an inactive branch take effect when you switch to it.

**Reset to defaults** removes the selected branch's overrides and restores the defaults above. Other branches retain their settings. Validation errors leave the form available to correct; failed saves restore the previous saved overrides and generated configurations.

Overrides are stored in `php-settings.json` inside the **Settings → Your files → Settings and database data** directory, outside the installed app. Only changed fields are saved, keyed by minor branch (`8.5`, for example). App and bundled PHP patch updates retain them; unmodified fields use the app's current defaults. Include this file when backing up your settings.

The managed `config/php.ini` is regenerated from defaults plus these overrides. Direct edits to that generated file are temporary. This UI covers the settings listed above; it does not manage arbitrary INI directives or extensions. Linux uses the same configuration backend but has no PHP-settings editor in its terminal dashboard.
