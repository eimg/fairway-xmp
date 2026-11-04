import assert from 'node:assert/strict';
import test from 'node:test';
import { searchGuides } from './search.js';

const entries = [
  { id: 'cli', title: 'Command line', text: 'Install the agent skill. fairway status --json' },
  { id: 'agent-skill', title: 'Install the bundled agent skill', text: 'Copy fairway-xmp/SKILL.md and reload the agent.' },
  { id: 'mysql', title: 'Connect MySQL', text: 'DB_PORT=13306 and database backups.' },
  { id: 'mm', title: 'စတင်အသုံးပြုရန်', text: 'Agent App တွင် Skill ကို ထည့်သွင်းပါ။' }
];

test('title matches rank first; queries ignore case and repeated whitespace', () => {
  assert.deepEqual(searchGuides(entries, '  AGENT   skill ').map(item => item.id), ['agent-skill', 'cli', 'mm']);
});
test('all terms must match, including literal command flags and paths', () => {
  assert.deepEqual(searchGuides(entries, 'fairway --json').map(item => item.id), ['cli']);
  assert.equal(searchGuides(entries, 'fairway-xmp/SKILL.md')[0].id, 'agent-skill');
  assert.equal(searchGuides(entries, 'DB_PORT=13306')[0].id, 'mysql');
  assert.deepEqual(searchGuides(entries, 'agent MySQL'), []);
});
test('Myanmar text supports substring matching', () => {
  assert.equal(searchGuides(entries, 'ထည့်သွင်း')[0].id, 'mm');
});
test('blank and unmatched input produces no results', () => {
  for (const query of ['', '   ', '<script>alert(1)</script>']) assert.deepEqual(searchGuides(entries, query), []);
});
test('excerpts include the match even deep in a guide and stay bounded', () => {
  const text = 'Before '.repeat(100) + 'needle ' + 'After '.repeat(100);
  const [match] = searchGuides([{ id: 'long', title: 'Long guide', text }], 'needle');
  assert(match.excerpt.includes('needle'));
  assert(match.excerpt.startsWith('…'));
  assert(match.excerpt.endsWith('…'));
  assert(match.excerpt.length <= 182);
});
