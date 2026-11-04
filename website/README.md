# Fairway XMP documentation website

Task-based how-to guides in English and Myanmar,
published on GitHub Pages: [English](https://eimg.github.io/fairway-xmp/) ·
[မြန်မာ](https://eimg.github.io/fairway-xmp/mm/).
This is independent of the desktop application's UI
and requires only Node.js 24; there are no site dependencies to install.

## Try it locally

From the repository root:

```sh
npm run site:dev
```

Open **http://127.0.0.1:4173/fairway-xmp/**. The preview uses the repository prefix
so relative links behave as they will on GitHub Pages. English is the default;
use **EN / MM** in the header (guide drawer on mobile) to switch languages. Myanmar uses the standard
HTML language code `my`, with `mm` in the URL and language control.

```text
/fairway-xmp/           English documentation home
/fairway-xmp/mm/        Myanmar documentation home
```

Stop the server with Ctrl+C. Re-run the command after editing source files; this
preview does not watch files. For another port: `SITE_PORT=4174 npm run site:dev`.

## Edit and verify

### Source files and checks

- `guides.mjs`: paired English/Myanmar task guides and navigation groups; optional `navTitle` pairs provide short directory labels, and `agent` blocks render immediately below a guide heading.
- `content.mjs`: translated site labels, common-task links and connection reference.
- `build.mjs`: shared page templates and static output in ignored `dist/`.
- `styles.css`: responsive layout, including Myanmar typography.
- `assets/`: checked-in screenshots from the app's simulated UI.
- `downloads.json`: published installer version and direct GitHub asset URLs.
- `device.js` and `site.js`: device selection, download dialog and language links.
- `search.js`: local full-text search modal and title ranking, tested in `search.test.mjs`.
- `dialog.js`: shared focus containment and scroll locking for stacked dialogs.
- `theme.js`: saved light/dark preference with automatic system fallback before first paint.

```sh
npm run site:check  # build, check links/assets/anchors, device and search tests
npm run site:build # build only
```

Both languages are rendered as complete HTML. Navigation and language switching
work without JavaScript. A small script follows the documentation section
actually in view when changing language. At the top it clears any old chapter
anchor, so a previous TOC visit cannot pull the translated page back down.
No analytics, external fonts or CDN are needed.
Detailed links to repository guides are explicitly marked as English. Edit the paired
English/Myanmar entries in `guides.mjs` and the locale labels in `content.mjs`,
then rebuild and review both languages locally before publishing.

The home page starts directly with the title, compact common-task links and problem
links, including Getting started. The redundant description and inline search block
are omitted. Common tasks use short navigation labels with full titles in tooltips.
The short `#getting-started` procedure verifies the local server and directs readers
to either the document-root or .test-site guide, with a brief optional agent skill link.
The dedicated `#agent-skill` guide under Start here covers scope, installation from
`skills/fairway-xmp` in the source release, activation and an initial inspection request.
Detailed project, database, terminal
and lifecycle procedures stay in their own sections; the general introduction has
no chapter anchor.
The directory groups guides by the reader's goal: Start here, Serve project folders,
Work with data & email, Configure bundled tools, Fix a problem, Manage Fairway XMP,
and Connection reference. Full task titles remain in article headings. The directory
uses short translated `navTitle` labels when provided, with the full title in the
link tooltip. Group headings use subtle background bands to separate them from
plain link rows, including in Burmese. Links have distinct padded rows, hover/focus
states and a current-section highlight that follows reading position. The desktop
sidebar scrolls independently. At 760px and below, one 64px sticky header holds
the menu icon, 28px logo beside a compact wordmark, and search icon. The menu opens the grouped directory
in a full-height sidebar drawer. Download, GitHub, language and theme controls move
into its footer and return to the desktop header when resized. The drawer slides from the left, is limited to 320px / 85vw, and
keeps its header and footer fixed around the scrolling directory. A dimmed backdrop
separates it from the page, and reduced-motion preferences disable the transition.
The drawer contains keyboard focus, highlights the current chapter
and locks background scrolling. Chapter selection closes it and focuses the
destination heading. Escape, close and backdrop dismissal restore focus to the
menu button; switching to desktop closes it and restores scrolling. Without
JavaScript or dialog support, the collapsed directory in the introduction remains
available. All guides remain in static HTML.
Existing chapter IDs are retained so previously shared links still resolve. The management group includes `#cli`, with a brief translated For agents block above its procedure and a link to the dedicated skill installation guide.

Search opens a native modal from the single visible icon in the desktop sidebar
or mobile header. The modal focuses its input, traps Tab focus, and shows hints,
result counts, excerpts and no-match feedback in the current language. It searches
all guide text, including collapsed troubleshooting, commands and connection details.
Every query term must match without case sensitivity, with title matches first.
Arrow Down focuses results; arrow keys navigate them. Enter selects the first
result from the input or the focused link. Selection closes the modal and focuses
the destination heading. Escape, close and backdrop dismissal preserve reading
position and restore focus to the visible trigger; the query is retained. Clear
resets results and focuses the input. Search sends no requests to a server. Without
JavaScript or dialog support, its controls stay hidden and the full guide stays usable.
The shared dialog helper retains scroll locking through stacked drawer/download
modals and contains keyboard focus in the top dialog.

With `npm run site:dev` running, `npm run site:test:navigation` runs the language
scroll, search and keyboard regression checks in Chromium and WebKit (requires the repository's npm
dependencies and Playwright browsers). Set `SITE_BASE_URL` for another preview
address.

The header (drawer footer on mobile) has a light/dark toggle with sun and moon icons. It follows the system
preference until a reader toggles it, then saves the explicit light/dark choice
across reloads, tabs and language changes. There is no System option in the UI.
Absent, invalid or legacy System preferences use the automatic fallback. Storage
errors leave the current-page toggle usable. CSS follows the system preference
without JavaScript; the toggle stays hidden in that case. Its translated accessible
name and checked state identify dark mode, and the tooltip describes the next action.
Screenshots retain their original colors.

With the preview running, `npm run site:test:theme` checks both browser engines
for preference changes and persistence, storage/JavaScript fallbacks, text and code
contrast, download controls, and desktop/mobile layouts in both languages.

## Editorial approach

Describe the behavior of the release the documentation accompanies. Keep
development status, planned availability and release chronology in release notes,
rather than in user procedures or agent instructions.

Write for someone who has a specific task or is stuck, rather than introducing
every feature or explaining the app's architecture. The interface handles routine
discovery; the guide supplies the missing steps and decisions.

Use neutral headings and concise descriptions for introductions. State the scope
directly instead of addressing the reader with questions or conversational prompts.

Use **Fairway XMP** as the full product name in English prose. Fairway is a
separate product. Preserve literal CLI commands, paths, account names and URLs.
For Burmese revisions, use the owner's edited page headings and labels in
`content.mjs` as the style reference: concise Burmese with familiar English
technical terms such as App, Database, Port, Command and Screenshot. Keep exact
application labels in English and preserve the owner's manual translations.

The scope is configuring and operating Fairway XMP. Assume an existing project;
do not teach HTML, PHP, Laravel, Composer package workflows or application development.
Omit hello-world code, framework initialization, application-key generation,
migrations and generic project setup. Include application-side details only when
needed to connect to a Fairway XMP service or select the correct Fairway XMP setting,
such as SMTP credentials, the MySQL port or a site's public web root.

Start directly with the procedure. Add an introduction only for a prerequisite,
platform restriction or consequence that affects the action. Do not add generic
“Use this for…” explanations, reasons to learn a tool or introductions that restate
the title. Headings, results and supporting labels use neutral procedural language.

Each entry in `guides.mjs` has (optional `navTitle` labels must be paired in both languages):

- A Fairway XMP task title, such as **Assign a .test address to a project**.
- Numbered steps with exact UI labels; optional prerequisites or constraints.
- An **Expected result** describing the outcome in concrete terms.
- Relevant **Troubleshooting** disclosures, named after symptoms or messages.
- Optional Linux instructions, platform-specific commands, screenshots and next steps.

Keep instructions short, but include information needed to finish the task without
having to read preceding chapters. Link to related guides for separate tasks.
Put consequences beside the action that causes them, especially for imports,
restores, deleting files and stopping commands. Avoid tool inventories, version
badges, generic tips and architectural walkthroughs. The ports table is a small
reference after the guides. Technical repository documentation remains available
through a clearly labeled English link, rather than being the default next step.

Screenshots are collapsed under **Screenshot** so they do not interrupt the
steps. Captions identify the simulated desktop UI. Linux instructions and full
restore commands are also expandable. Troubleshooting, platform notes, command
examples and screenshots share bordered
summary rows with trailing chevrons and consistent collapsed spacing. Expanded
headers and padded content share a single bordered panel, separated by an inner
divider. Native disclosures, anchor links and the mobile directory work without
JavaScript.

Maintain matching task IDs, meaning and level of detail in English and Myanmar.
Keep UI labels in English to match the application. `site:check` validates the
translated guide structure, common-task targets, preserved anchors and local links.
The browser tests cover desktop/mobile task discovery without JavaScript, opening
contextual help, and language navigation while reading.

## Download dialog

**Get Fairway XMP** opens a native modal dialog in either language. Windows,
macOS and Linux browsers see their platform's installer as the main download,
with smaller links for the other platforms. Mobile, unrecognized devices and
known unsupported architectures see all three platform choices. Requirements
are always shown: browser detection cannot reliably identify every CPU or OS
version, and a Mac user agent does not distinguish Intel from Apple Silicon.

The dialog supports Escape, a close button and clicking the backdrop, contains
keyboard focus, and returns focus to the header button when closed. Without
JavaScript or native dialog support, the header links to GitHub Releases.

The current installer URLs were verified against the published v0.1.1 release.
Update `downloads.json` and translated requirements in `content.mjs` when
publishing a new supported release; local builds never need a GitHub API call.

## Refresh screenshots

The eight screenshots show the actual app interface with simulated sample data,
not a running native environment. Captions identify this on both languages.
To refresh, install the repository's normal npm dependencies and Playwright
Chromium if needed, start the app's UI-test server in one terminal, and capture
in another:

```sh
npm run dev:ui -- --port 1421
# Another terminal:
node website/capture-screenshots.mjs
# Refresh only Terminal and Logs:
node website/capture-screenshots.mjs --workspaces-only
# Refresh only Sites, Settings and Updates:
node website/capture-screenshots.mjs --app-guides-only
```

The capture script hides the fixture toolbar for composition. It does not start
native services or access real user data. Review the images before committing.

## Publish to GitHub Pages

The `Documentation website` workflow validates relevant pull requests. Deployment is
manual and restricted to `main`.

The repository uses **GitHub Actions** as its Pages source. After pushing reviewed
changes to `main`, run **Actions → Documentation website → Run workflow** from
`main` to update the [published guides](https://eimg.github.io/fairway-xmp/).
See [GitHub's custom workflow documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

Upload only `website/dist`, not the desktop app's root `dist`. The site uses
relative asset and navigation URLs, so it also works at a custom domain root.
