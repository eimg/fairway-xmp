// The editor sends text segments only; original HTML and technical tokens stay fixed.
const protectedTags = new Set(['strong', 'code', 'pre', 'kbd']);
const escape = text => text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
export function splitTranslation(text, rich = false) {
  if (!rich) return [{ text }];
  let depth = 0;
  return text.split(/(<[^>]*>)/g).map(part => {
    if (part.startsWith('<')) {
      const tag = part.match(/^<\/?([a-z]+)/i)?.[1]?.toLowerCase();
      if (protectedTags.has(tag)) depth += part.startsWith('</') ? -1 : 1;
      return { html: part };
    }
    return depth ? { html: part } : { text: part };
  });
}
export function replaceTranslation(original, values, rich = false) {
  const parts = splitTranslation(original, rich);
  if (!Array.isArray(values) || values.length !== parts.filter(part => 'text' in part).length ||
      values.some(value => typeof value !== 'string' || value.length > 100000 || value.includes('\0'))) {
    throw new Error('Invalid translation text. Reload the editor and try again.');
  }
  let index = 0;
  return parts.map(part => 'html' in part ? part.html : rich ? escape(values[index++]) : values[index++]).join('');
}
export function translationCatalog({ copy, groups, guides }) {
  const sections = [
    { id: 'page', title: 'Page headings and labels' },
    { id: 'navigation', title: 'Navigation groups' },
    ...guides.map(guide => ({ id: guide.id, title: guide.title[0] })),
  ];
  const fields = [];
  function add(id, section, label, english, translation, write, rich = false) {
    fields.push({ id, section, label, english, translation, rich, parts: splitTranslation(translation, rich), write });
  }
  function walk(english, translation, path = []) {
    if (['lang', 'quickTasks', 'quickFixes'].includes(path[0])) return;
    if (path[0] === 'portRows' && path[2] === '1') return;
    if (typeof english === 'string') {
      let parent = copy.mm;
      for (const key of path.slice(0, -1)) parent = parent[key];
      const key = path.at(-1);
      const labels = { overview: 'Page title', intro: 'Introduction', check: 'Expected result label', stuck: 'Troubleshooting heading', theme: 'Theme toggle', downloadDialog: 'Download dialog', portRows: 'Connection table', portHeaders: 'Connection table headings' };
      const label = path.map(part => labels[part] || (/^\d+$/.test(part) ? Number(part) + 1 : part.replace(/([a-z])([A-Z])/g, '$1 $2'))).join(' · ');
      add(`page.${path.join('.')}`, 'page', label, english, translation, value => { parent[key] = value; }, path[0] === 'portHelp');
    } else {
      for (const key of Object.keys(english)) walk(english[key], translation[key], [...path, key]);
    }
  }
  walk(copy.en, copy.mm);
  for (const group of groups) add(`group.${group.id}`, 'navigation', group.name[0], group.name[0], group.name[1], value => { group.name[1] = value; });
  for (const guide of guides) {
    const paired = (key, label, rich = true) => {
      if (guide[key]) add(`${guide.id}.${key}`, guide.id, label, guide[key][0], guide[key][1], value => { guide[key][1] = value; }, rich);
    };
    paired('title', 'Guide title', false);
    paired('intro', 'Prerequisite or constraint');
    guide.steps[0].forEach((step, index) => add(`${guide.id}.step.${index}`, guide.id, `Step ${index + 1}`, step, guide.steps[1][index], value => { guide.steps[1][index] = value; }, true));
    paired('check', 'Expected result');
    guide.trouble?.forEach(([title, body], index) => {
      add(`${guide.id}.trouble.${index}.title`, guide.id, `Troubleshooting ${index + 1} · heading`, title[0], title[1], value => { title[1] = value; });
      add(`${guide.id}.trouble.${index}.body`, guide.id, `Troubleshooting ${index + 1} · text`, body[0], body[1], value => { body[1] = value; }, true);
    });
    paired('linux', 'Linux instructions');
    guide.platformCommands?.forEach((platform, index) => {
      add(`${guide.id}.platform.${index}.title`, guide.id, `Restore command ${index + 1} · heading`, platform.title[0], platform.title[1], value => { platform.title[1] = value; });
      if (platform.body) add(`${guide.id}.platform.${index}.body`, guide.id, `Restore command ${index + 1} · instructions`, platform.body[0], platform.body[1], value => { platform.body[1] = value; }, true);
    });
  }
  return { sections, fields };
}
