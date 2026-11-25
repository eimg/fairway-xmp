import { createContext, useContext, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Maximize2, X } from 'lucide-react';
import { useModalFocus } from './modal-focus';
import './workspace.css';

type WorkspaceId = 'postgres' | 'node-logs' | 'terminal' | 'logs' | 'mail' | 'data';
type Expanded = { id: WorkspaceId; height: number; trigger: HTMLElement | null };
const Context = createContext<{ expanded: Expanded | null; maximize: (id: WorkspaceId) => void; restore: () => void } | null>(null);

export function WorkspaceProvider({ children }: { children: ReactNode }) {
  const [expanded, setExpanded] = useState<Expanded | null>(null);
  return <Context.Provider value={{ expanded, maximize: id => {
    const root = document.querySelector<HTMLElement>(`[data-workspace="${id}"]`);
    if (root) setExpanded({ id, height: root.getBoundingClientRect().height, trigger: document.activeElement instanceof HTMLElement ? document.activeElement : null });
  }, restore: () => setExpanded(null) }}>{children}</Context.Provider>;
}

export function useMaximizedWorkspace() { return useContext(Context)?.expanded?.id ?? null; }
export function useRestoreWorkspace() { return useContext(Context)!.restore; }

export function MaximizeWorkspace({ id, title }: { id: WorkspaceId; title: string }) {
  const context = useContext(Context)!;
  return <button className="icon-button workspace-maximize" aria-label={`Maximize ${title}`} title={`Maximize ${title}`} onClick={event => { event.currentTarget.focus({ preventScroll: true }); context.maximize(id); }}><Maximize2 size={16} /></button>;
}

export default function Workspace({ id, title, className, actions, children, busy }: {
  id: WorkspaceId; title: string; className: string; actions?: ReactNode; children: ReactNode; busy?: boolean;
}) {
  const context = useContext(Context)!;
  const maximized = context.expanded?.id === id;
  const root = useRef<HTMLDivElement>(null);
  useModalFocus(root, maximized, context.restore, { workspace: true, returnFocus: context.expanded?.trigger });
  return <div className="workspace-slot" style={maximized ? { height: context.expanded!.height } : undefined}>
    <div ref={root} className={`${className}${maximized ? ' workspace-maximized' : ''}`} data-workspace={id} role={maximized ? 'dialog' : undefined} aria-modal={maximized || undefined} aria-label={maximized ? `${title} maximized` : undefined} aria-busy={busy} tabIndex={maximized ? -1 : undefined}>
      <div className="workspace-maximized-header" hidden={!maximized}><h2>{title}</h2><div className="workspace-maximized-actions">{actions}<button className="icon-button workspace-restore" aria-label={`Restore ${title}`} title={id === 'terminal' ? 'Restore view (⌘/Ctrl+Shift+M; Esc outside terminal input)' : id === 'mail' ? 'Restore view (Esc outside the email preview)' : 'Restore view (Esc)'} onClick={context.restore}><X size={19} /></button></div></div>
      {children}
    </div>
  </div>;
}

// App-level operation feedback stays actionable inside the active workspace.
export function WorkspaceFeedback({ children }: { children: ReactNode }) {
  const context = useContext(Context)!;
  const host = context.expanded ? document.querySelector(`[data-workspace="${context.expanded.id}"]`) : null;
  return host ? createPortal(children, host) : children;
}
