<?php
$escape = static fn (string $value): string => htmlspecialchars($value, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
$documentRoot = (string) ($_SERVER['DOCUMENT_ROOT'] ?? __DIR__);
$port = (int) ($_SERVER['SERVER_PORT'] ?? 8800);
if ($port < 1 || $port > 65535) {
    $port = 8800;
}
$exampleUrl = 'http://127.0.0.1:' . $port . '/project-name/';
?>
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Fairway XMP — Local server</title>
  <style>
    :root { color-scheme: light; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; color: #253b32; background: #f5f7f4; }
    * { box-sizing: border-box; }
    body { margin: 0; padding: clamp(32px, 9vh, 88px) 24px; }
    main { max-width: 720px; margin: 0 auto; }
    .brand { display: flex; align-items: center; gap: 12px; margin-bottom: 32px; }
    .brand-mark { display: grid; place-items: center; width: 42px; height: 42px; background: #e5efea; color: #28745d; border-radius: 12px; }
    .brand-name { font-size: 28px; font-weight: 750; letter-spacing: -1.3px; line-height: 1; }
    .brand-name span { color: #28745d; }
    .brand-edition { display: block; margin-top: 6px; color: #28745d; font-size: 11px; font-weight: 650; letter-spacing: .04em; }
    .welcome { background: #fff; border: 1px solid #e1e8e2; border-radius: 18px; padding: clamp(26px, 5vw, 48px); }
    h1 { max-width: 520px; margin: 0; font-size: clamp(30px, 4.5vw, 42px); font-weight: 650; line-height: 1.15; letter-spacing: -1.4px; }
    .intro { margin: 16px 0 32px; color: #63756a; font-size: 16px; line-height: 1.7; }
    h2 { margin: 0 0 10px; font-size: 14px; font-weight: 650; }
    code { font: 13px/1.7 ui-monospace, SFMono-Regular, Consolas, monospace; overflow-wrap: anywhere; }
    .root { display: block; padding: 14px 16px; border: 1px solid #e4ebe6; border-radius: 8px; background: #f6f9f6; color: #355945; }
    .start { margin-top: 28px; }
    .start p { margin: 0; color: #63756a; font-size: 14px; line-height: 1.8; }
    .start p + p { margin-top: 10px; }
    .start p code { color: #355945; }
    .example { display: block; margin-top: 10px; color: #355945; }
    .app-hint { margin: 28px 0 0; color: #627266; font-size: 13px; line-height: 1.7; }
    nav { display: flex; flex-wrap: wrap; gap: 14px 24px; padding-top: 26px; margin-top: 30px; border-top: 1px solid #e8ede9; }
    a { display: inline-flex; align-items: center; gap: 6px; color: #28745d; text-decoration: none; font-size: 14px; font-weight: 550; padding: 4px 0; }
    a:hover { text-decoration: underline; text-underline-offset: 4px; }
    a:focus-visible { outline: 3px solid #91c7b4; outline-offset: 5px; border-radius: 3px; }
    .external { font-weight: 400; }
    footer { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px 20px; padding: 20px 4px 0; color: #637367; font-size: 12px; line-height: 1.7; }
    @media (max-width: 420px) { body { padding: 28px 16px; } .welcome { padding: 26px 22px; } h1 { letter-spacing: -1px; } nav { flex-direction: column; gap: 10px; } }
  </style>
</head>
<body>
  <main>
    <header class="brand" aria-label="Fairway XMP">
      <span class="brand-mark" aria-hidden="true">
        <svg width="25" height="25" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.48 19 2c1 2 2 4.18 2 8 0 5.5-4.78 10-10 10Z"/>
          <path d="M2 21c0-3 1.85-5.36 5.08-6C9.5 14.52 12 13 13 12"/>
        </svg>
      </span>
      <div><div class="brand-name">fairway<span>.</span></div><span class="brand-edition">XMP</span></div>
    </header>
    <section class="welcome" aria-labelledby="welcome-title">
      <h1 id="welcome-title">Local server</h1>
      <p class="intro">Nginx is serving this page through PHP.</p>
      <h2>Document root</h2>
      <code class="root"><?= $escape($documentRoot) ?></code>
      <div class="start">
        <h2>Open a project</h2>
        <p>Place a PHP project folder inside the document root. For a folder named <code>project-name</code>, open:</p>
        <code class="example"><?= $escape($exampleUrl) ?></code>
      </div>
      <p class="app-hint">To serve a project from another location, use <strong>Manage Sites → Add site</strong> in Fairway XMP. For Laravel, keep the project outside this document root and select its <code>public</code> folder as the web root.</p>
      <div class="start">
        <h2>Command-line tools</h2>
        <p>Open <strong>Terminal</strong> in Fairway XMP to run PHP, Composer and <code>fairway</code> commands. Run <code>fairway --help</code> to list commands, or <code>fairway status</code> to check services, ports and sites.</p>
        <p>For an external terminal or editor, enable <strong>Settings → Command-line tools</strong>, then open a new terminal or restart the editor.</p>
      </div>
      <nav aria-label="Helpful links">
        <a href="https://eimg.github.io/fairway-xmp/#getting-started">Getting started <span class="external" aria-hidden="true">↗</span></a>
        <a href="https://eimg.github.io/fairway-xmp/#cli">CLI guide <span class="external" aria-hidden="true">↗</span></a>
        <a href="https://www.php.net/manual/en/">PHP manual <span class="external" aria-hidden="true">↗</span></a>
        <a href="https://github.com/eimg/fairway-xmp/issues">Get help <span class="external" aria-hidden="true">↗</span></a>
      </nav>
    </section>
    <footer><span>Served locally by Fairway XMP</span><span>PHP <?= $escape(PHP_VERSION) ?></span></footer>
  </main>
</body>
</html>
