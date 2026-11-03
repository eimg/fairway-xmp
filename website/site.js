import { detectPlatform } from './device.js';

// The URL hash can outlive a visit to a chapter after a manual scroll.
// Link to the chapter actually in view, or the top when reading the introduction.
const languageLinks = [...document.querySelectorAll('.languages a')];
const chapters = [...document.querySelectorAll('.docs-content > section[id]')];
const chapterLinks = [...document.querySelectorAll('.toc-group a')];
function updateChapterNavigation() {
  const readingLine = Math.min(128, window.innerHeight / 4);
  const chapter = chapters.find(section => {
    const bounds = section.getBoundingClientRect();
    return bounds.top <= readingLine && bounds.bottom > readingLine;
  });
  languageLinks.forEach(link => { link.hash = chapter ? `#${chapter.id}` : ''; });
  chapterLinks.forEach(link => {
    if (chapter && link.hash === `#${chapter.id}`) link.setAttribute('aria-current', 'location');
    else link.removeAttribute('aria-current');
  });
}
let navigationFrame = null;
function scheduleChapterNavigation() {
  if (navigationFrame !== null) return;
  navigationFrame = requestAnimationFrame(() => {
    navigationFrame = null;
    updateChapterNavigation();
  });
}
languageLinks.forEach(link => link.addEventListener('click', updateChapterNavigation));
updateChapterNavigation();
window.addEventListener('scroll', scheduleChapterNavigation, { passive: true });
window.addEventListener('resize', scheduleChapterNavigation);
window.addEventListener('hashchange', scheduleChapterNavigation);
window.addEventListener('pageshow', scheduleChapterNavigation);

const trigger = document.querySelector('[data-open-downloads]');
const dialog = document.querySelector('#download-dialog');
if (trigger && dialog && typeof dialog.showModal === 'function') {
  const platform = detectPlatform(navigator);
  const options = [...dialog.querySelectorAll('[data-platform]')];
  const selected = options.find(option => option.dataset.platform === platform);
  const primary = dialog.querySelector('[data-download-primary]');
  if (selected) {
    const link = primary.querySelector('a');
    primary.querySelector('[data-primary-name]').textContent = selected.querySelector('.download-platform-name').textContent;
    primary.querySelector('[data-primary-requirements]').textContent = selected.querySelector('.download-requirements').textContent;
    link.href = selected.href;
    link.querySelector('[data-primary-label]').textContent = selected.dataset.downloadLabel;
    primary.hidden = false;
    selected.hidden = true;
    dialog.querySelector('[data-download-other-title]').textContent = dialog.dataset.otherPlatforms;
    dialog.querySelector('[data-download-intro]').textContent = dialog.dataset.detectedIntro;
  }
  let previousOverflow = null;
  trigger.addEventListener('click', () => {
    dialog.showModal();
    if (previousOverflow === null) previousOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
  });
  dialog.addEventListener('close', () => {
    // Native close events are queued; a quick reopen must keep the original value.
    if (!dialog.open && previousOverflow !== null) {
      document.body.style.overflow = previousOverflow;
      previousOverflow = null;
      // Safari does not focus a button on pointer click before opening a dialog.
      trigger.focus({ preventScroll: true });
    }
  });
  dialog.addEventListener('click', event => {
    const bounds = dialog.getBoundingClientRect();
    if (event.target === dialog && (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom)) dialog.close();
  });
  trigger.hidden = false;
  document.querySelector('[data-download-fallback]').hidden = true;
}
