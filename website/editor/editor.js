const entries = document.querySelector('#entries');
const section = document.querySelector('#section');
const search = document.querySelector('#search');
const saveButton = document.querySelector('#save');
const download = document.querySelector('#download');
const importButton = document.querySelector('#import');
const fileInput = document.querySelector('#draft-file');
const status = document.querySelector('#status');
const changes = new Map();
let catalog;
let saving = false;
const escape = text => text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
const decode = text => { const area = document.createElement('textarea'); area.innerHTML = text; return area.value; };
const draftKey = () => `fairway-burmese-draft:${catalog.revision}`;
const originals = field => field.parts.filter(part => 'text' in part).map(part => field.rich ? decode(part.text) : part.text);
function message(text, error = false) { status.textContent = text; status.classList.toggle('error', error); }
function updateButtons() {
  saveButton.disabled = saving || !changes.size;
  download.disabled = saving || !changes.size;
  importButton.disabled = saving;
  section.disabled = saving;
  search.disabled = saving;
  entries.inert = saving;
}
function rememberDraft() {
  try {
    if (changes.size) localStorage.setItem(draftKey(), JSON.stringify(draft()));
    else localStorage.removeItem(draftKey());
  } catch { /* Save and draft download still work without browser storage. */ }
}
function draft() {
  return { version: 1, revision: catalog.revision, changes: [...changes].map(([id, values]) => ({ id, english: catalog.fields.find(field => field.id === id).english, values })) };
}
function importDraft(value) {
  if (value.version !== 1 || !Array.isArray(value.changes)) throw new Error('This is not a Fairway XMP translation draft.');
  let applied = 0, skipped = 0;
  for (const change of value.changes) {
    const field = catalog.fields.find(field => field.id === change.id);
    if (!field || field.english !== change.english || !Array.isArray(change.values) || change.values.length !== originals(field).length || change.values.some(text => typeof text !== 'string')) { skipped++; continue; }
    if (JSON.stringify(change.values) === JSON.stringify(originals(field))) changes.delete(field.id);
    else { changes.set(field.id, change.values); applied++; }
  }
  return { applied, skipped };
}
function render() {
  const query = search.value.trim().toLocaleLowerCase();
  const visible = catalog.fields.filter(field => query
    ? [field.english, ...(changes.get(field.id) || originals(field))].join(' ').toLocaleLowerCase().includes(query)
    : field.section === section.value);
  entries.replaceChildren();
  for (const field of visible) {
    const row = document.createElement('section');
    row.className = `entry${changes.has(field.id) ? ' changed' : ''}`;
    row.dataset.field = field.id;
    const heading = document.createElement('h2');
    heading.textContent = query ? `${catalog.sections.find(item => item.id === field.section).title} · ${field.label}` : field.label;
    const body = document.createElement('div'); body.className = 'entry-body';
    const reference = document.createElement('div'); reference.className = 'reference';
    if (field.rich) reference.innerHTML = field.english; else reference.textContent = field.english;
    const translation = document.createElement('div'); translation.className = 'translation'; translation.lang = 'my';
    const values = changes.get(field.id) || originals(field);
    let index = 0;
    translation.innerHTML = field.parts.map(part => {
      if ('html' in part) return part.html;
      const current = index++;
      return `<span contenteditable="plaintext-only" role="textbox" aria-multiline="true" aria-label="${escape(field.label)} · Burmese text ${current + 1}" data-segment="${current}" spellcheck="false">${escape(values[current])}</span>`;
    }).join('');
    body.append(reference, translation); row.append(heading, body); entries.append(row);
  }
  document.querySelector('#count').textContent = `${visible.length} text fields`;
  document.querySelector('#empty').hidden = !!visible.length;
  document.querySelector('#preview').href = `/preview/mm/${['page', 'navigation'].includes(section.value) ? '' : `#${section.value}`}`;
  updateButtons();
}
async function load() {
  const response = await fetch('/api/catalog');
  const value = await response.json();
  if (!response.ok) throw new Error(value.error);
  catalog = value;
  const selected = section.value;
  section.replaceChildren(...catalog.sections.map(item => { const option = document.createElement('option'); option.value = item.id; option.textContent = item.title; return option; }));
  if (catalog.sections.some(item => item.id === selected)) section.value = selected;
}
entries.addEventListener('input', event => {
  const row = event.target.closest('.entry');
  if (!row) return;
  const field = catalog.fields.find(field => field.id === row.dataset.field);
  const values = [...row.querySelectorAll('[data-segment]')].map(span => span.innerText.replaceAll('\r\n', '\n'));
  if (JSON.stringify(values) === JSON.stringify(originals(field))) changes.delete(field.id);
  else changes.set(field.id, values);
  row.classList.toggle('changed', changes.has(field.id));
  rememberDraft(); updateButtons();
  message(changes.size ? `${changes.size} unsaved text fields. Save & update preview applies them to the guide.` : 'No unsaved changes.');
});
// Reference links are displayed for context; editing must not navigate away.
entries.addEventListener('click', event => { if (event.target.closest('a')) event.preventDefault(); });
section.addEventListener('change', render);
search.addEventListener('input', render);
saveButton.addEventListener('click', async () => {
  if (saving || !changes.size) return;
  saving = true; updateButtons(); message('Saving Burmese translations and updating the preview…');
  const previousKey = draftKey();
  try {
    const response = await fetch('/api/save', {
      method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Editor-Token': catalog.token },
      body: JSON.stringify({ revision: catalog.revision, changes: [...changes].map(([id, values]) => ({ id, values })) }),
    });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error);
    await load(); changes.clear();
    try { localStorage.removeItem(previousKey); } catch { /* Draft storage is optional. */ }
    render(); message(`Saved ${result.saved} Burmese text fields. The saved guide preview is updated.`);
  } catch (error) { message(error.message, true); }
  finally { saving = false; updateButtons(); }
});
download.addEventListener('click', () => {
  const link = document.createElement('a');
  link.href = URL.createObjectURL(new Blob([JSON.stringify(draft(), null, 2)], { type: 'application/json' }));
  link.download = `fairway-burmese-draft-${new Date().toISOString().slice(0, 10)}.json`;
  link.click(); setTimeout(() => URL.revokeObjectURL(link.href), 1000);
});
importButton.addEventListener('click', () => fileInput.click());
fileInput.addEventListener('change', async () => {
  const file = fileInput.files[0];
  if (!file) return;
  try {
    if (file.size > 1024 * 1024) throw new Error('Draft files must be smaller than 1 MiB.');
    const result = importDraft(JSON.parse(await file.text()));
    rememberDraft(); render(); message(`Imported ${result.applied} edits. ${result.skipped ? `${result.skipped} outdated fields were skipped. ` : ''}Review and save to apply them.`);
  } catch (error) { message(error.message, true); }
  fileInput.value = '';
});
window.addEventListener('beforeunload', event => { if (changes.size) { event.preventDefault(); event.returnValue = ''; } });
document.addEventListener('keydown', event => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') { event.preventDefault(); if (!saveButton.disabled) saveButton.click(); }
});
try {
  await load();
  let restored = 0;
  try { const saved = localStorage.getItem(draftKey()); if (saved) restored = importDraft(JSON.parse(saved)).applied; } catch { /* An invalid or unavailable draft does not block editing. */ }
  render(); message(restored ? `Restored ${restored} unsaved text fields from this browser. Review and save to apply them.` : 'Ready. Select a section or find a phrase, then click the Burmese text to edit.');
} catch (error) { message(error.message, true); }
