// Check the published HTML, including links resolved under a GitHub project path.
import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { copy } from './content.mjs';
import { groups, guides } from './guides.mjs';

const root = new URL('./dist/', import.meta.url);
const base = new URL('https://example.test/fairway-xmp/');
const routes = ['', 'mm/'];
let checked = 0;
assert.equal(new Set(guides.map(guide => guide.id)).size, guides.length, 'unique task IDs');
for (const guide of guides) {
  assert(groups.some(group => group.id === guide.group), `${guide.id}: known task group`);
  for (const i of [0, 1]) {
    assert(guide.title[i]?.length, `${guide.id}: translated title`);
    if (guide.intro) assert(guide.intro[i]?.length, `${guide.id}: translated prerequisite or constraint`);
    assert(guide.steps[i]?.length >= 2 && guide.steps[i].every(step => step.length), `${guide.id}: translated steps`);
    assert(guide.check[i]?.length, `${guide.id}: translated result check`);
    for (const [title, body] of guide.trouble || []) assert(title[i]?.length && body[i]?.length, `${guide.id}: translated troubleshooting`);
    if (guide.linux) assert(guide.linux[i]?.length, `${guide.id}: translated Linux guidance`);
  }
  for (const id of guide.related || []) assert(id === 'default-ports' || guides.some(item => item.id === id), `${guide.id}: valid next step ${id}`);
}
for (const route of routes) {
  const html = await readFile(new URL(`${route}index.html`, root), 'utf8');
  assert.match(html, route.startsWith('mm/') ? /<html lang="my">/ : /<html lang="en">/);
  assert.equal([...html.matchAll(/<h1[ >]/g)].length, 1, `${route}: one main heading`);
  const ids = [...html.matchAll(/\bid="([^"]+)"/g)].map(match => match[1]);
  assert.equal(new Set(ids).size, ids.length, `${route}: unique fragment IDs`);
  assert.match(html, /class="docs-layout wrap"/, `${route}: documentation is the home page`);
  assert.doesNotMatch(html, /class="hero|class="service-grid/, `${route}: landing page removed`);
  const i = route ? 1 : 0;
  const locale = route ? 'mm' : 'en';
  for (const guide of guides) {
    assert(ids.includes(guide.id), `${route}: missing ${guide.id}`);
    assert(html.includes(`<h2><a class="heading-link" href="#${guide.id}">${guide.title[i]}</a></h2>`), `${route}: translated task heading`);
  }
  for (const id of ['getting-started', 'lifecycle', 'nginx', 'php', 'mysql', 'phpmyadmin', 'mailpit', 'composer', 'terminal', 'logs', 'sites', 'settings', 'updates', 'ports', 'default-ports']) assert(ids.includes(id), `${route}: preserved section link ${id}`);
  for (const id of [...copy[locale].quickTasks, ...copy[locale].quickFixes]) assert(guides.some(guide => guide.id === id), `${route}: valid home task`);
  assert.match(html, /class="mobile-contents"/, `${route}: mobile task directory`);
  assert.doesNotMatch(html, /Worth knowing|Core services|How a request becomes a page/, `${route}: task-first structure`);
  for (const group of groups.map(group => group.id)) assert(ids.includes(`toc-${group}`), `${route}: missing TOC group ${group}`);
  for (const match of html.matchAll(/\b(?:href|src)="([^"]+)"/g)) {
    const link = new URL(match[1], new URL(route, base));
    if (link.origin !== base.origin) continue;
    assert(link.pathname.startsWith(base.pathname), `${route}: link escapes project prefix: ${link}`);
    const relative = link.pathname.slice(base.pathname.length);
    const path = !relative || relative.endsWith('/') ? `${relative}index.html` : relative;
    assert((await stat(new URL(path, root))).isFile(), `${route}: missing ${path}`);
    if (link.hash) {
      const target = await readFile(new URL(path, root), 'utf8');
      assert(target.includes(`id="${decodeURIComponent(link.hash.slice(1))}"`), `${route}: missing anchor ${link}`);
    }
    checked++;
  }
  for (const image of html.matchAll(/<img\b[^>]+>/g)) assert.match(image[0], /\balt="[^"]*"/, `${route}: image alt text`);
}
console.log(`Checked ${routes.length} bilingual pages and ${checked} local links, assets and fragments under /fairway-xmp/.`);
