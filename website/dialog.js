// Keep scrolling locked until the last modal closes, including downloads opened
// from the mobile drawer. Release synchronously before following chapter links.
const active = new Set();
const initialized = new WeakSet();
let originalOverflow = '';

function release(dialog) {
  if (active.delete(dialog) && !active.size) document.body.style.overflow = originalOverflow;
}

export function openDialog(dialog) {
  if (dialog.open) return;
  if (!initialized.has(dialog)) {
    dialog.addEventListener('close', () => { if (!dialog.open) release(dialog); });
    dialog.addEventListener('cancel', event => {
      if (event.defaultPrevented) return;
      event.preventDefault();
      closeDialog(dialog);
    });
    dialog.addEventListener('keydown', event => {
      if (event.key !== 'Tab' || event.ctrlKey || event.metaKey || event.defaultPrevented) return;
      const controls = [...dialog.querySelectorAll('a[href], button, input, select, textarea, summary, [tabindex]')]
        .filter(element => !element.disabled && element.tabIndex >= 0 && element.getClientRects().length);
      if (!controls.length) return;
      event.preventDefault();
      const index = controls.indexOf(document.activeElement);
      const next = index < 0 ? (event.shiftKey ? controls.length - 1 : 0) : (index + (event.shiftKey ? -1 : 1) + controls.length) % controls.length;
      controls[next].focus();
    });
    initialized.add(dialog);
  }
  dialog.showModal();
  if (!active.size) originalOverflow = document.body.style.overflow;
  active.add(dialog);
  document.body.style.overflow = 'hidden';
}

export function closeDialog(dialog) {
  dialog.close();
  release(dialog);
}

export function isBackdropClick(dialog, event) {
  const bounds = dialog.getBoundingClientRect();
  return event.target === dialog && (event.clientX < bounds.left || event.clientX > bounds.right || event.clientY < bounds.top || event.clientY > bounds.bottom);
}
