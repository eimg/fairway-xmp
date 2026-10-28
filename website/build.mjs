import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { copy } from './content.mjs';
import { groups, guides } from './guides.mjs';

const out = new URL('./dist/', import.meta.url);
const repo = 'https://github.com/eimg/fairway-xmp';
const downloads = JSON.parse(await readFile(new URL('./downloads.json', import.meta.url), 'utf8'));
const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
const code = value => `<pre><code>${escape(value)}</code></pre>`;
const arrow = '<span aria-hidden="true">↗</span>';
const downloadIcon = '<svg class="download-icon" xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>';

await rm(out, { recursive: true, force: true });
await mkdir(out, { recursive: true });
await cp(new URL('./assets/', import.meta.url), new URL('assets/', out), { recursive: true });
await cp(new URL('./styles.css', import.meta.url), new URL('styles.css', out));
await cp(new URL('../src-tauri/icons/128x128.png', import.meta.url), new URL('assets/icon.png', out));
await writeFile(new URL('.nojekyll', out), '');
for (const file of ['site.js', 'device.js', 'theme.js']) await cp(new URL(file, import.meta.url), new URL(file, out));

for (const locale of ['en', 'mm']) {
  const t = copy[locale];
  const i = locale === 'en' ? 0 : 1;
  const home = locale === 'en' ? '' : 'mm/';
  const dir = home;
  const root = '../'.repeat(dir.split('/').filter(Boolean).length) || './';
  const homeUrl = root + home;
  const asset = file => `${root}assets/${file}`;
  const image = (file, caption, eager = false) => `<figure class="screenshot"><a href="${asset(file)}" aria-label="${escape(caption)}"><img src="${asset(file)}" alt="${escape(caption)}" width="1280" height="960" ${eager ? 'fetchpriority="high"' : 'loading="lazy"'}></a><figcaption>${escape(caption)}</figcaption></figure>`;
  const languages = ['en', 'mm'].map(lang => {
    const route = lang === 'en' ? '' : 'mm/';
    return `<a href="${root}${route}" lang="${copy[lang].lang}" hreflang="${copy[lang].lang}" ${lang === locale ? 'aria-current="page"' : ''}>${lang === 'en' ? 'EN' : 'MM'}<span class="sr-only"> · ${escape(copy[lang].label)}</span></a>`;
  }).join('<span aria-hidden="true">/</span>');
  const themeControl = `<button type="button" class="theme-toggle" role="switch" aria-checked="false" aria-label="${escape(t.theme.label)}" title="${escape(t.theme.toDark)}" data-theme-control data-theme-toggle data-to-dark="${escape(t.theme.toDark)}" data-to-light="${escape(t.theme.toLight)}" hidden><svg class="theme-sun" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.4 1.4m11.2 11.2L19 19M5 19l1.4-1.4M17.6 6.4 19 5"/></svg><svg class="theme-moon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20.9 13A9 9 0 0 1 11 3.1 9 9 0 1 0 20.9 13Z"/></svg></button>`;
  const header = `<a class="skip" href="#main">${escape(t.skip)}</a><header class="header wrap"><a class="brand" href="${homeUrl}" aria-label="Fairway XMP"><img src="${asset('icon.png')}" width="38" height="38" alt=""><span>fairway<span class="brand-dot">.</span><small>XMP</small></span></a><nav aria-label="${locale === 'en' ? 'Main navigation' : 'အဓိက လမ်းညွှန်'}"><button type="button" class="download-trigger" data-open-downloads aria-haspopup="dialog" aria-controls="download-dialog" hidden>${escape(t.download)}</button><a class="download-trigger" data-download-fallback href="${downloads.releaseUrl}">${escape(t.download)}</a><a class="github" href="${repo}">${escape(t.nav.github)} ${arrow}</a></nav><div class="header-preferences"><div class="languages" aria-label="Language / ဘာသာစကား">${languages}</div>${themeControl}</div></header>`;
  const footer = `<footer class="footer wrap"><div><a class="footer-brand" href="${homeUrl}">fairway<span>.</span> XMP</a><p>${escape(t.footer)}</p></div><div><a href="${repo}">${escape(t.source)} ${arrow}</a><p>${escape(t.scope)}</p></div></footer>`;
  const titleFor = id => id === 'default-ports' ? t.ports : guides.find(guide => guide.id === id).title[i];
  const taskLinks = ids => ids.map(id => `<li><a href="#${id}">${escape(titleFor(id))}</a></li>`).join('');
  const renderToc = (suffix = '') => groups.map(group => {
    const items = group.id === 'reference' ? [{ id: 'default-ports', title: [copy.en.ports, copy.mm.ports] }] : guides.filter(guide => guide.group === group.id);
    return `<div class="toc-group"><h2 id="toc-${group.id}${suffix}">${escape(group.name[i])}</h2><ul aria-labelledby="toc-${group.id}${suffix}">${items.map(item => `<li><a href="#${item.id}">${escape(item.title[i])}</a></li>`).join('')}</ul></div>`;
  }).join('');
  const renderGuide = guide => `<section id="${guide.id}" class="task-guide">
    <p class="eyebrow guide-category">${escape(groups.find(group => group.id === guide.group).name[i])}</p>
    <h2><a class="heading-link" href="#${guide.id}">${escape(guide.title[i])}</a></h2>
    ${guide.intro ? `<p>${guide.intro[i]}</p>` : ''}
    <ol class="guide-steps">${guide.steps[i].map(step => `<li>${step}</li>`).join('')}</ol>
    ${guide.platformCommands?.map(platform => `<details class="guide-platform"><summary>${escape(platform.title[i])}</summary>${platform.body ? `<p>${platform.body[i]}</p>` : ''}${code(platform.command)}</details>`).join('') || ''}
    <div class="guide-result"><h3>${escape(t.check)}</h3><p>${guide.check[i]}</p></div>
    ${guide.trouble?.length ? `<div class="guide-troubleshooting"><h3>${escape(t.troubleshooting)}</h3>${guide.trouble.map(([title, body]) => `<details><summary>${title[i]}</summary><p>${body[i]}</p></details>`).join('')}</div>` : ''}
    ${guide.linux ? `<details class="guide-platform"><summary>${escape(t.linuxTitle)}</summary><div class="platform-body">${guide.linux[i]}</div></details>` : ''}
    ${guide.image ? `<details class="guide-screenshot"><summary>${escape(t.screenshotLabel)}</summary>${image(guide.image, t.screenshot)}</details>` : ''}
    ${guide.related?.length ? `<nav class="guide-related" aria-label="${escape(t.related)}"><h3>${escape(t.related)}</h3><ul>${taskLinks(guide.related)}</ul></nav>` : ''}
    </section>`;
  const content = `
    <div class="docs-layout wrap"><aside class="docs-sidebar"><p class="eyebrow">${escape(t.contents)}</p><nav aria-label="${escape(t.contents)}">${renderToc()}</nav><a class="sidebar-source" href="${repo}/tree/main/docs">${escape(t.fullDocs)} ${arrow}</a></aside><article class="docs-content">
    <section id="getting-started" class="guide-home"><h1>${escape(t.overview)}</h1><p>${escape(t.intro)}</p>
    <nav class="common-tasks" aria-labelledby="popular-tasks"><h2 id="popular-tasks">${escape(t.popular)}</h2><ul>${taskLinks(t.quickTasks)}</ul></nav>
    <nav class="common-fixes" aria-labelledby="common-problems"><h2 id="common-problems">${escape(t.stuck)}</h2><ul>${taskLinks(t.quickFixes)}</ul></nav>
    <details class="mobile-contents"><summary>${escape(t.contents)}</summary><nav aria-label="${escape(t.contents)}">${renderToc('-mobile')}</nav></details><p class="scope-note">${escape(t.scopeNote)}</p></section>
    ${guides.map(renderGuide).join('')}
    <section id="default-ports"><p class="eyebrow guide-category">${escape(groups.find(group => group.id === 'reference').name[i])}</p><h2>${escape(t.ports)}</h2><p>${escape(t.portNote)}</p><div class="table-scroll" role="region" aria-label="${escape(t.ports)}" tabindex="0"><table><thead><tr>${t.portHeaders.map(header => `<th scope="col">${escape(header)}</th>`).join('')}</tr></thead><tbody>${t.portRows.map(row => `<tr><th scope="row">${escape(row[0])}</th><td><code>${escape(row[1])}</code></td><td>${escape(row[2])}</td></tr>`).join('')}</tbody></table></div><p>${t.portHelp}</p></section>
    <section class="next"><h2>${escape(t.nextTitle)}</h2><p>${escape(t.nextText)}</p><a class="text-link" href="${repo}/issues/new">${escape(t.help)} ${arrow}</a></section></article></div>`;
  const d = t.downloadDialog;
  const downloadDialog = `<dialog id="download-dialog" class="download-dialog" aria-labelledby="download-title" aria-describedby="download-intro" data-other-platforms="${escape(d.other)}" data-detected-intro="${escape(d.detectedIntro)}">
    <div class="download-content"><form method="dialog" class="download-close-form"><button class="download-close" aria-label="${escape(d.close)}" autofocus><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg></button></form>
    <p class="eyebrow download-release">FAIRWAY XMP · ${escape(d.version)} ${escape(downloads.version)}</p><h2 id="download-title">${escape(t.download)}</h2><p id="download-intro" class="download-intro" data-download-intro>${escape(d.intro)}</p>
    <div class="download-primary" data-download-primary hidden><p class="eyebrow">${escape(d.suggested)}</p><h3 data-primary-name></h3><p class="download-requirements" data-primary-requirements></p><a class="download-main-button" href="${downloads.releaseUrl}"><span data-primary-label></span>${downloadIcon}</a></div>
    <h3 class="download-other-title" data-download-other-title>${escape(d.choose)}</h3><div class="download-options">${downloads.platforms.map(platform => `<a class="download-option" data-platform="${platform.id}" data-download-label="${escape(`${d.button} ${platform.name}`)}" href="${escape(platform.url)}"><span><strong class="download-platform-name">${platform.name}</strong><small class="download-requirements">${escape(d.requirements[platform.id])} · ${platform.format}</small></span>${downloadIcon}</a>`).join('')}</div>
    <a class="download-release-link" href="${downloads.releaseUrl}">${escape(d.releaseNotes)} ${arrow}</a></div></dialog>`;
  const title = `${t.nav.docs} — Fairway XMP`;
  const html = `<!doctype html>
<html lang="${escape(t.lang)}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>${escape(title)}</title><meta name="description" content="${escape(t.description)}"><meta name="color-scheme" content="light dark"><meta name="theme-color" content="#f7f8f2"><script src="${root}theme.js"></script><meta property="og:title" content="${escape(title)}"><meta property="og:description" content="${escape(t.description)}"><meta property="og:type" content="website"><link rel="icon" href="${asset('icon.png')}"><link rel="stylesheet" href="${root}styles.css">${['en', 'mm'].map(lang => `<link rel="alternate" hreflang="${copy[lang].lang}" href="${root}${lang === 'en' ? '' : 'mm/'}">`).join('')}</head><body>${header}<main id="main">${content}</main>${footer}${downloadDialog}<script src="${root}site.js" type="module"></script></body></html>`;
  await mkdir(new URL(dir, out), { recursive: true });
  await writeFile(new URL(`${dir}index.html`, out), html);
}
console.log('Built English and Myanmar documentation homes in website/dist.');
