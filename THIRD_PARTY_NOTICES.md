# Third-party software

Fairway XMP bundles independently licensed software. The Fairway XMP application code is MIT licensed; that license does not replace the licenses of bundled components.

| Component | License / upstream |
|---|---|
| PHP | PHP License 3.01 and Zend Engine license; https://www.php.net/license/ |
| MySQL Community Server | GPL v2 with Universal FOSS Exception; https://github.com/mysql/mysql-server |
| Nginx | BSD 2-Clause; https://nginx.org/LICENSE |
| phpMyAdmin | GPL v2 or later; https://github.com/phpmyadmin/phpmyadmin |
| Composer | MIT; https://github.com/composer/composer |
| Tauri | MIT / Apache 2.0; https://github.com/tauri-apps/tauri |
| React | MIT; https://github.com/facebook/react |
| Lucide | ISC; https://github.com/lucide-icons/lucide |
| xterm.js, Fit and Search addons | MIT; https://github.com/xtermjs/xterm.js |
| portable-pty | MIT; https://github.com/wezterm/wezterm/tree/main/pty |

Runtime staging retains upstream license, notice, and copying files under `runtime/licenses` and within component directories. Microsoft Visual C++ and WebView2 are separately licensed prerequisites and remain on uninstall if shared with other applications.

Source code for bundled tools is available alongside the installers on the [Releases page](https://github.com/eimg/fairway-xmp/releases). The source archive includes version-matched sources, build recipes and patches, and component licenses. The application source is available in this repository. For build and redistribution instructions, see [Contributing](CONTRIBUTING.md).

Linux additionally bundles native libraries used by PHP, Nginx and MySQL. Component-specific library copies use their upstream licenses; Ubuntu library copyright notices are under `runtime/licenses/linux-libraries`, and exact source package/version records are in `runtime/linux-library-sources.json`. The Linux corresponding-source artifact contains their source packages, Debian/Ubuntu patches, and the build recipes. MySQL’s bundled third-party library notices remain in its distribution.
