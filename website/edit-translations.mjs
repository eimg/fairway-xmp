// Local-only authoring server. This file and the editor assets are never published.
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, rename, stat } from 'node:fs/promises';
import { createHash, randomBytes } from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve, sep } from 'node:path';
import { translationCatalog, replaceTranslation } from './translation-data.mjs';

const here = new URL('./', import.meta.url);
let port = Number(process.env.SITE_EDITOR_PORT || 4174);
const token = randomBytes(32).toString('hex');
const sourceNames = ['content.mjs', 'guides.mjs'];
let saving = false;
const revisionFor = sources => createHash('sha256').update(sources.join('\0')).digest('hex');
async function load() {
  const sources = await Promise.all(sourceNames.map(name => readFile(new URL(name, here), 'utf8')));
  const revision = revisionFor(sources);
  const [{ copy }, { groups, guides }] = await Promise.all(sourceNames.map(name => import(new URL(`${name}?revision=${revision}`, here))));
  const model = structuredClone({ copy, groups, guides });
  return { sources, revision, model, ...translationCatalog(model) };
}
async function build() {
  await new Promise((accept, reject) => {
    const child = spawn(process.execPath, [fileURLToPath(new URL('build.mjs', here))], { stdio: ['ignore', 'pipe', 'pipe'] });
    let output = '';
    for (const stream of [child.stdout, child.stderr]) stream.on('data', data => { output = (output + data).slice(-10000); });
    child.once('error', reject);
    child.once('close', code => code === 0 ? accept() : reject(new Error(`Preview build failed: ${output}`)));
  });
}
async function atomic(name, value) {
  const temporary = new URL(`${name}.editor-${token}.tmp`, here);
  await writeFile(temporary, value);
  await rename(temporary, new URL(name, here));
}
async function save(payload) {
  if (saving) throw new Error('A save is already in progress. Try again shortly.');
  saving = true;
  try {
    const current = await load();
    if (payload.revision !== current.revision) throw new Error('The guide changed outside this editor. Download the draft, then reload before saving.');
    if (!Array.isArray(payload.changes) || payload.changes.length > current.fields.length) throw new Error('Invalid changes.');
    const seen = new Set();
    for (const change of payload.changes) {
      const field = current.fields.find(field => field.id === change.id);
      if (!field || seen.has(change.id)) throw new Error('Unknown or duplicate translation field.');
      seen.add(change.id);
      const value = replaceTranslation(field.translation, change.values, field.rich);
      if (!value.trim()) throw new Error(`${field.label} cannot be empty.`);
      field.write(value);
    }
    if (!seen.size) return { revision: current.revision, saved: 0 };
    const backupName = `${new Date().toISOString().replaceAll(':', '-')}-${randomBytes(3).toString('hex')}`;
    const backup = new URL(`.translation-backups/${backupName}/`, here);
    await mkdir(backup, { recursive: true });
    await Promise.all(sourceNames.map((name, index) => writeFile(new URL(name, backup), current.sources[index])));
    const headers = current.sources.map(source => source.slice(0, source.indexOf('export const')));
    const updated = [
      `${headers[0]}export const copy = ${JSON.stringify(current.model.copy, null, 2)};\n`,
      `${headers[1]}export const groups = ${JSON.stringify(current.model.groups, null, 2)};\n\nexport const guides = ${JSON.stringify(current.model.guides, null, 2)};\n`,
    ];
    try {
      for (const [index, name] of sourceNames.entries()) await atomic(name, updated[index]);
      await build();
    } catch (error) {
      for (const [index, name] of sourceNames.entries()) await atomic(name, current.sources[index]);
      await build().catch(() => {});
      throw new Error(`Save failed; original translations were restored. ${error.message}`);
    }
    return { revision: revisionFor(updated), saved: seen.size };
  } finally { saving = false; }
}
const types = { '.html': 'text/html; charset=utf-8', '.css': 'text/css; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.png': 'image/png' };
function reply(res, status, value) {
  res.writeHead(status, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
  res.end(JSON.stringify(value));
}
const server = createServer(async (req, res) => {
  const host = req.headers.host;
  if (![ `127.0.0.1:${port}`, `localhost:${port}` ].includes(host)) { reply(res, 403, { error: 'Local editor access only.' }); return; }
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.setHeader('X-Frame-Options', 'DENY');
  const path = new URL(req.url, `http://${host}`).pathname;
  try {
    if (req.method === 'POST' && path === '/api/save') {
      if (req.headers.origin !== `http://${host}` || req.headers['x-editor-token'] !== token || !req.headers['content-type']?.startsWith('application/json')) {
        reply(res, 403, { error: 'Save requests must come from this editor.' }); return;
      }
      const chunks = [];
      let size = 0;
      for await (const part of req) {
        size += part.length;
        if (size > 1024 * 1024) throw new Error('Draft is too large. Save fewer fields at once.');
        chunks.push(part);
      }
      reply(res, 200, await save(JSON.parse(Buffer.concat(chunks).toString('utf8')))); return;
    }
    if (req.method !== 'GET') { reply(res, 405, { error: 'Method not allowed.' }); return; }
    if (path === '/api/catalog') {
      const data = await load();
      reply(res, 200, { token, revision: data.revision, sections: data.sections, fields: data.fields.map(({ write, ...field }) => field) }); return;
    }
    let file;
    if (path.startsWith('/preview/')) {
      const root = resolve(fileURLToPath(new URL('dist/', here)));
      const relative = decodeURIComponent(path.slice('/preview/'.length));
      file = resolve(root, relative);
      if (file !== resolve(root) && !file.startsWith(root + sep)) { reply(res, 403, { error: 'Invalid preview path.' }); return; }
      if ((await stat(file)).isDirectory()) {
        if (!path.endsWith('/')) { res.writeHead(302, { Location: path + '/' }); res.end(); return; }
        file = resolve(file, 'index.html');
      }
    } else {
      const assets = { '/': 'editor/index.html', '/editor.js': 'editor/editor.js', '/editor.css': 'editor/editor.css' };
      if (!assets[path]) { reply(res, 404, { error: 'Not found.' }); return; }
      file = fileURLToPath(new URL(assets[path], here));
    }
    const body = await readFile(file);
    res.writeHead(200, { 'Content-Type': types[file.slice(file.lastIndexOf('.'))] || 'application/octet-stream', 'Cache-Control': 'no-store' });
    res.end(body);
  } catch (error) { reply(res, 400, { error: error.message || 'Unable to complete the request.' }); }
});
await build();
server.listen(port, '127.0.0.1', () => {
  port = server.address().port;
  console.log(`Burmese editor: http://127.0.0.1:${port}/\nEdit text, then choose Save & update preview. Ctrl+C stops the editor.`);
});
server.on('error', error => { console.error(error.message); process.exitCode = 1; });
