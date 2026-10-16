import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { CircleHelp } from 'lucide-react';

export default function HelpTip({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const id = useId();
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) setOpen(false); };
    const key = (event: KeyboardEvent) => { if (event.key === 'Escape') setOpen(false); };
    document.addEventListener('pointerdown', outside); document.addEventListener('keydown', key);
    return () => { document.removeEventListener('pointerdown', outside); document.removeEventListener('keydown', key); };
  }, [open]);
  return <div ref={root} className="help-tip" onPointerEnter={() => setOpen(true)} onPointerLeave={() => { if (!root.current?.contains(document.activeElement)) setOpen(false); }} onBlur={event => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setOpen(false); }}>
    <button className="icon-button" aria-label={label} aria-describedby={open ? id : undefined} onFocus={() => setOpen(true)} onClick={() => setOpen(true)}><CircleHelp size={16} /></button>
    {open && <div role="tooltip" id={id} className="help-tip-content">{children}</div>}
  </div>;
}
