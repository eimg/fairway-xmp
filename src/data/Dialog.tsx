import { useEffect, useId, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';

export default function Dialog({ title, busy, onClose, onSubmit, children, submit = 'Save', danger = false, valid = true }: {
  title: string; busy: boolean; onClose: () => void; onSubmit: () => void;
  children: ReactNode; submit?: string; danger?: boolean; valid?: boolean;
}) {
  const container = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  const pending = useRef(busy);
  const label = useId();
  useEffect(() => { close.current = onClose; pending.current = busy; }, [onClose, busy]);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    container.current?.querySelector<HTMLElement>('input, textarea, select, button')?.focus();
    function key(event: KeyboardEvent) {
      if (event.key === 'Escape' && !pending.current) close.current();
      if (event.key !== 'Tab') return;
      const items = container.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href]');
      if (!items?.length) return;
      const first = items[0], last = items[items.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }
    document.addEventListener('keydown', key);
    return () => { document.removeEventListener('keydown', key); previous?.focus(); };
  }, []);
  return <div className="modal-backdrop"><div ref={container} className="modal db-dialog" role="dialog" aria-modal="true" aria-labelledby={label}>
    <form onSubmit={event => { event.preventDefault(); if (!busy && valid) onSubmit(); }}>
      <div className="card-title"><h2 id={label}>{title}</h2></div>
      <fieldset disabled={busy}>{children}</fieldset>
      <div className="db-dialog-actions"><button type="button" className="secondary" onClick={onClose} disabled={busy}><X size={14} />Cancel</button><button type="submit" className={danger ? 'db-danger' : 'primary'} disabled={busy || !valid}>{busy ? 'Working…' : submit}</button></div>
    </form>
  </div></div>;
}
