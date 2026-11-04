import { closeDialog, isBackdropClick, openDialog } from './dialog.js';

const compact = value => value.normalize('NFC').replace(/\s+/gu, ' ').trim();
const normalize = value => compact(value).toLowerCase();

// Search the current translation, including text inside collapsed disclosures.
export function searchGuides(entries, query) {
  const terms = [...new Set(normalize(query).split(' ').filter(Boolean))];
  if (!terms.length) return [];
  return entries.flatMap((entry, order) => {
    const title = normalize(entry.title);
    const text = compact(entry.text);
    const body = normalize(text);
    if (!terms.every(term => `${title} ${body}`.includes(term))) return [];
    const score = terms.reduce((sum, term) => sum + (title.includes(term) ? 10 : 0), 0);
    const position = Math.min(...terms.map(term => body.indexOf(term)).filter(index => index >= 0));
    const start = Number.isFinite(position) ? Math.max(0, position - 60) : 0;
    const excerpt = `${start ? '…' : ''}${text.slice(start, start + 180)}${text.length > start + 180 ? '…' : ''}`;
    return [{ ...entry, excerpt, score, order }];
  }).sort((a, b) => b.score - a.score || a.order - b.order);
}

export function initGuideSearch() {
  const panel = document.querySelector('[data-guide-search]');
  if (!panel || typeof panel.showModal !== 'function') return;
  const input = panel.querySelector('input');
  const results = panel.querySelector('ul');
  const status = panel.querySelector('[role="status"]');
  const clear = panel.querySelector('[type="reset"]');
  let opener = null;
  let destination = null;
  const entries = [...document.querySelectorAll('.docs-content > section[id]')].map(section => {
    const content = section.cloneNode(true);
    content.querySelectorAll('h2, .guide-category, .guide-related, figure').forEach(node => node.remove());
    // Separate adjacent block elements so queries work across markup boundaries.
    content.querySelectorAll('p, li, h3, summary, pre, th, td').forEach(node => node.append(' '));
    return { id: section.id, title: section.querySelector('h2').textContent, text: content.textContent };
  });
  function update() {
    const query = input.value.trim();
    results.replaceChildren();
    results.hidden = !query;
    clear.hidden = !query;
    if (!query) { status.textContent = panel.dataset.hint; return; }
    const matches = searchGuides(entries, query);
    status.textContent = matches.length ? panel.dataset.count.replace('{count}', matches.length) : panel.dataset.empty;
    for (const match of matches) {
      const item = document.createElement('li');
      const link = document.createElement('a');
      link.href = `#${match.id}`;
      link.textContent = match.title;
      link.addEventListener('click', event => {
        if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
        destination = document.querySelector(`#${match.id} h2`);
        closeDialog(panel);
      });
      const excerpt = document.createElement('p');
      excerpt.textContent = match.excerpt;
      item.append(link, excerpt);
      results.append(item);
    }
  }
  input.addEventListener('input', update);
  panel.querySelector('form').addEventListener('submit', event => {
    event.preventDefault();
    results.querySelector('a')?.click();
  });
  input.addEventListener('keydown', event => {
    if (event.key === 'ArrowDown' && results.querySelector('a')) {
      event.preventDefault();
      results.querySelector('a').focus();
    }
  });
  results.addEventListener('keydown', event => {
    if (!['ArrowDown', 'ArrowUp'].includes(event.key)) return;
    const links = [...results.querySelectorAll('a')];
    const index = links.indexOf(document.activeElement);
    if (index < 0) return;
    event.preventDefault();
    const next = event.key === 'ArrowDown' ? Math.min(index + 1, links.length - 1) : index - 1;
    (next < 0 ? input : links[next]).focus();
  });
  panel.querySelector('form').addEventListener('reset', event => {
    event.preventDefault();
    input.value = '';
    update();
    input.focus();
  });
  document.querySelectorAll('[data-guide-search-link]').forEach(link => {
    link.hidden = false;
    link.addEventListener('click', () => {
      opener = link;
      destination = null;
      openDialog(panel);
      input.focus({ preventScroll: true });
    });
  });
  panel.querySelector('[data-close-search]').addEventListener('click', () => closeDialog(panel));
  panel.addEventListener('keydown', event => {
    if (event.key === 'Escape') {
      // A search input can otherwise consume Escape just to clear its value.
      event.preventDefault();
      event.stopPropagation();
      closeDialog(panel);
    }
  });
  panel.addEventListener('click', event => { if (isBackdropClick(panel, event)) closeDialog(panel); });
  panel.addEventListener('close', () => {
    if (panel.open) return;
    if (destination) {
      const heading = destination;
      requestAnimationFrame(() => {
        heading.tabIndex = -1;
        heading.focus({ preventScroll: true });
      });
    } else {
      const visibleOpener = opener?.getClientRects().length ? opener : [...document.querySelectorAll('[data-guide-search-link]')].find(link => link.getClientRects().length);
      visibleOpener?.focus({ preventScroll: true });
    }
  });
}
