import { detectPlatform } from './device.js';
import { initGuideSearch } from './search.js';
import { closeDialog, isBackdropClick, openDialog } from './dialog.js';

initGuideSearch();

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

const guideMenu = document.querySelector('#guide-menu');
const menuTrigger = document.querySelector('[data-guide-menu-trigger]');
if (guideMenu && menuTrigger && typeof guideMenu.showModal === 'function') {
  const mobile = matchMedia('(max-width: 760px)');
  const header = document.querySelector('.header');
  const actions = header.querySelector('nav');
  const preferences = header.querySelector('.header-preferences');
  const actionHome = document.createComment('header actions');
  const preferenceHome = document.createComment('header preferences');
  actions.before(actionHome);
  preferences.before(preferenceHome);
  let destination = null;
  function closeMenu() {
    closeDialog(guideMenu);
    menuTrigger.setAttribute('aria-expanded', 'false');
  }
  menuTrigger.addEventListener('click', () => {
    destination = null;
    updateChapterNavigation();
    openDialog(guideMenu);
    menuTrigger.setAttribute('aria-expanded', 'true');
    const current = guideMenu.querySelector('[aria-current="location"]');
    const nav = guideMenu.querySelector('.guide-menu-directory');
    if (current) nav.scrollTop += current.getBoundingClientRect().top - nav.getBoundingClientRect().top - nav.clientHeight / 2;
  });
  guideMenu.querySelector('[data-guide-menu-close]').addEventListener('click', closeMenu);
  guideMenu.addEventListener('cancel', event => { event.preventDefault(); closeMenu(); });
  guideMenu.addEventListener('close', () => {
    // A queued close event must not affect a menu that has already reopened.
    if (guideMenu.open) return;
    if (destination) {
      const heading = destination;
      requestAnimationFrame(() => {
        heading.tabIndex = -1;
        heading.focus({ preventScroll: true });
      });
    } else if (mobile.matches) menuTrigger.focus({ preventScroll: true });
  });
  guideMenu.addEventListener('click', event => {
    const link = event.target.closest('a[href^="#"]');
    if (link && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey) {
      destination = document.querySelector(`${link.hash} h2`);
      closeMenu();
    }
    if (isBackdropClick(guideMenu, event)) closeMenu();
  });
  function syncHeader() {
    if (mobile.matches) {
      guideMenu.querySelector('[data-menu-actions]').append(actions);
      guideMenu.querySelector('[data-menu-preferences]').append(preferences);
    } else {
      if (guideMenu.open) closeMenu();
      actionHome.after(actions);
      preferenceHome.after(preferences);
    }
  }
  mobile.addEventListener('change', syncHeader);
  syncHeader();
  menuTrigger.hidden = false;
  document.documentElement.setAttribute('data-guide-menu-ready', '');
  document.querySelector('.mobile-contents').hidden = true;
}

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
  trigger.addEventListener('click', () => openDialog(dialog));
  dialog.addEventListener('close', () => {
    // Native close events are queued; a quick reopen must keep the original value.
    if (!dialog.open) {
      // Safari does not focus a button on pointer click before opening a dialog.
      trigger.focus({ preventScroll: true });
    }
  });
  dialog.addEventListener('click', event => {
    if (isBackdropClick(dialog, event)) closeDialog(dialog);
  });
  trigger.hidden = false;
  document.querySelector('[data-download-fallback]').hidden = true;
}
