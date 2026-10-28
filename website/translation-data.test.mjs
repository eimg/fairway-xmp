import assert from 'node:assert/strict';
import test from 'node:test';
import { copy } from './content.mjs';
import { groups, guides } from './guides.mjs';
import { splitTranslation, replaceTranslation, translationCatalog } from './translation-data.mjs';

test('rich edits preserve fixed labels, command blocks, link targets and HTML structure', () => {
  const original = '<strong>Environment</strong> တွင် <code>fairway/www</code> ကို <a href="#sites">ဖွင့်ပါ</a>။<pre><code>php --version\ncomposer --version</code></pre>';
  const parts = splitTranslation(original, true);
  const values = parts.filter(part => 'text' in part).map(part => part.text.replace('ဖွင့်ပါ', 'ကြည့်ပါ').replace(' တွင် ', ' မှ '));
  const updated = replaceTranslation(original, values, true);
  assert(updated.includes('<strong>Environment</strong> မှ <code>fairway/www</code>'));
  assert(updated.includes('<a href="#sites">ကြည့်ပါ</a>'));
  assert(updated.includes('<pre><code>php --version\ncomposer --version</code></pre>'));
});

test('typed markup remains text and malformed edits are rejected', () => {
  assert.equal(replaceTranslation('စာသား', ['<script>alert(1)</script> & မြန်မာ'], true), '&lt;script&gt;alert(1)&lt;/script&gt; &amp; မြန်မာ');
  assert.throws(() => replaceTranslation('စာသား', [], true));
  assert.throws(() => replaceTranslation('စာသား', [null], true));
  assert.throws(() => replaceTranslation('စာသား', ['\0'], true));
});

test('the catalog writes Burmese only, including optional instructions and site labels', () => {
  const model = structuredClone({ copy, groups, guides });
  const before = structuredClone(model);
  const data = translationCatalog(model);
  assert.equal(data.fields.length, new Set(data.fields.map(field => field.id)).size);
  assert.equal(data.sections.length, guides.length + 2);
  assert(!data.fields.some(field => /quickTasks|quickFixes|\.lang$|portRows\.\d+\.1$/.test(field.id)));
  for (const field of data.fields) {
    assert.equal(typeof field.english, 'string');
    assert.equal(typeof field.translation, 'string');
    field.write(field.translation + ' စမ်းသပ်');
  }
  assert.deepEqual(model.copy.en, before.copy.en);
  assert.deepEqual(model.copy.mm.quickTasks, before.copy.mm.quickTasks);
  assert.equal(model.copy.mm.lang, 'my');
  for (const [index, guide] of model.guides.entries()) {
    assert.deepEqual(guide.steps[0], before.guides[index].steps[0]);
    assert.equal(guide.title[0], before.guides[index].title[0]);
    assert.deepEqual(guide.related, before.guides[index].related);
    guide.platformCommands?.forEach((platform, part) => assert.equal(platform.command, before.guides[index].platformCommands[part].command));
  }
  assert(model.copy.mm.overview.endsWith(' စမ်းသပ်'));
});
