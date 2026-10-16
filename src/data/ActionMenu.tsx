import { useEffect, useId, useRef, useState } from 'react';
import { Ellipsis } from 'lucide-react';

type Action = { label: string; onSelect: () => void; disabled?: boolean; danger?: boolean };

export default function ActionMenu({ label, disabled, actions }: { label: string; disabled: boolean; actions: Action[] }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menuId = useId();
  useEffect(() => {
    if (!open) return;
    root.current?.querySelector<HTMLButtonElement>('[role=menuitem]:not(:disabled)')?.focus();
    function outside(event: PointerEvent) { if (!root.current?.contains(event.target as Node)) setOpen(false); }
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);
  return <div className="db-action-menu" ref={root} onKeyDown={event => {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); setOpen(false); trigger.current?.focus(); return; }
    if (!open) { if (event.key === 'ArrowDown') { event.preventDefault(); setOpen(true); } return; }
    if (event.key === 'Tab') { setOpen(false); return; }
    const items = Array.from(root.current?.querySelectorAll<HTMLButtonElement>('[role=menuitem]:not(:disabled)') ?? []);
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const index = event.key === 'ArrowDown' ? (current + 1) % items.length : event.key === 'ArrowUp' ? (current - 1 + items.length) % items.length : event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : null;
    if (index !== null) { event.preventDefault(); items[index]?.focus(); }
  }} onBlur={event => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setOpen(false); }}>
    <button ref={trigger} className="icon-button" aria-label={label} title={label} aria-haspopup="menu" aria-expanded={open} aria-controls={open ? menuId : undefined} disabled={disabled} onMouseDown={event => event.preventDefault()} onClick={() => { setOpen(!open); if (open) trigger.current?.focus(); }}><Ellipsis size={18} /></button>
    {/* macOS WebKit blurs a focused menu item when another button is clicked,
        without focusing the clicked button. Keep focus until its click runs. */}
    {open && <div className="db-action-popup" role="menu" aria-label={label} id={menuId} onMouseDown={event => event.preventDefault()}>{actions.map(action => <button key={action.label} role="menuitem" className={action.danger ? 'db-menu-danger' : ''} disabled={disabled || action.disabled} onClick={() => { setOpen(false); trigger.current?.focus(); action.onSelect(); }}>{action.label}</button>)}</div>}
  </div>;
}
