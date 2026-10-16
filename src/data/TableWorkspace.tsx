import { useEffect, useId, useRef, useState, type CSSProperties, type PointerEvent, type ReactNode } from 'react';
import { PanelLeftClose, PanelLeftOpen } from 'lucide-react';

const defaultWidth = 180;
const minimumWidth = 120;

export default function TableWorkspace({ sidebar, children }: { sidebar: ReactNode; children: (toggle: ReactNode) => ReactNode }) {
  const container = useRef<HTMLDivElement>(null);
  const drag = useRef<{ id: number; startX: number; startWidth: number } | null>(null);
  const [availableWidth, setAvailableWidth] = useState(0);
  const [preferredWidth, setPreferredWidth] = useState(defaultWidth);
  const [collapsed, setCollapsed] = useState(false);
  const [resizing, setResizing] = useState(false);
  const sidebarId = useId();
  // Leave room for the data panel, including at the minimum desktop window size.
  const maximumWidth = availableWidth ? Math.max(minimumWidth, Math.min(480, Math.floor(availableWidth * .6), availableWidth - 290)) : 480;
  const width = Math.max(minimumWidth, Math.min(preferredWidth, maximumWidth));
  const clamp = (value: number) => Math.max(minimumWidth, Math.min(Math.round(value), maximumWidth));

  useEffect(() => {
    const element = container.current;
    if (!element) return;
    const observer = new ResizeObserver(entries => setAvailableWidth(entries[0].contentRect.width));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  function endDrag(event: PointerEvent<HTMLDivElement>) {
    if (drag.current?.id !== event.pointerId) return;
    drag.current = null; setResizing(false);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }
  const toggle = <button className={`${collapsed ? 'secondary' : 'icon-button'} db-sidebar-toggle`} aria-label={collapsed ? 'Show tables' : 'Hide tables'} title={collapsed ? 'Show tables' : 'Hide tables'} aria-expanded={!collapsed} aria-controls={sidebarId} onClick={() => setCollapsed(value => !value)}>{collapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}{collapsed && <span>Show tables</span>}</button>;

  return <div ref={container} className={`db-workspace${collapsed ? ' db-sidebar-collapsed' : ''}${resizing ? ' db-resizing' : ''}`} style={{ '--db-sidebar-width': `${width}px` } as CSSProperties}>
    <div className="db-sidebar-pane" id={sidebarId} hidden={collapsed}>{sidebar}</div>
    {!collapsed && <div className="db-sidebar-divider" role="separator" aria-label="Resize table sidebar" aria-orientation="vertical" aria-controls={sidebarId} aria-valuemin={minimumWidth} aria-valuemax={maximumWidth} aria-valuenow={width} aria-valuetext={`${width} pixels`} tabIndex={0}
      title="Drag to resize tables. Double-click to reset."
      onPointerDown={event => {
        if (event.button !== 0 || !event.isPrimary) return;
        event.preventDefault(); event.currentTarget.focus();
        event.currentTarget.setPointerCapture(event.pointerId);
        drag.current = { id: event.pointerId, startX: event.clientX, startWidth: width }; setResizing(true);
      }}
      onPointerMove={event => { if (drag.current?.id === event.pointerId) setPreferredWidth(clamp(drag.current.startWidth + event.clientX - drag.current.startX)); }}
      onPointerUp={endDrag} onPointerCancel={endDrag} onLostPointerCapture={() => { drag.current = null; setResizing(false); }}
      onDoubleClick={() => setPreferredWidth(defaultWidth)}
      onKeyDown={event => {
        const step = event.shiftKey ? 40 : 16;
        const next = event.key === 'ArrowLeft' ? width - step : event.key === 'ArrowRight' ? width + step : event.key === 'Home' ? minimumWidth : event.key === 'End' ? maximumWidth : null;
        if (next !== null) { event.preventDefault(); setPreferredWidth(clamp(next)); }
      }}
    ><span /></div>}
    {children(toggle)}
  </div>;
}
