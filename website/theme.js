// Runs before the stylesheet so a saved preference is applied before first paint.
(() => {
  const key = 'fairway-docs-theme';
  const explicit = value => value === 'light' || value === 'dark';
  const system = window.matchMedia('(prefers-color-scheme: dark)');
  let preference = null;
  try {
    const saved = localStorage.getItem(key);
    if (explicit(saved)) preference = saved;
  } catch { /* System preference still works when storage is unavailable. */ }
  const isDark = () => preference === 'dark' || (preference === null && system.matches);

  function apply() {
    if (preference) document.documentElement.dataset.theme = preference;
    else delete document.documentElement.dataset.theme;
    const dark = isDark();
    document.querySelector('meta[name="theme-color"]')?.setAttribute('content', dark ? '#121c18' : '#f7f8f2');
    const toggle = document.querySelector('[data-theme-toggle]');
    if (toggle) {
      toggle.setAttribute('aria-checked', String(dark));
      toggle.title = dark ? toggle.dataset.toLight : toggle.dataset.toDark;
    }
  }
  apply();
  system.addEventListener('change', apply);
  window.addEventListener('storage', event => {
    if (event.key !== key && event.key !== null) return;
    preference = explicit(event.newValue) ? event.newValue : null;
    apply();
  });
  document.addEventListener('DOMContentLoaded', () => {
    const toggle = document.querySelector('[data-theme-toggle]');
    if (!toggle) return;
    apply();
    toggle.addEventListener('click', () => {
      preference = isDark() ? 'light' : 'dark';
      apply();
      try { localStorage.setItem(key, preference); } catch { /* Keep this page usable. */ }
    });
    toggle.hidden = false;
  });
})();
