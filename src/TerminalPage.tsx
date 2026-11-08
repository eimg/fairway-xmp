import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import { BookOpen, Check, ChevronDown, ChevronUp, Clipboard, Copy, Eraser, ExternalLink, Minus, MoreHorizontal, Plus, RefreshCw, Search, Terminal as TerminalIcon, X } from 'lucide-react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { canControl, invoke, isUiTest } from './bridge';
import GuideLink from './GuideLink';
import Workspace, { MaximizeWorkspace } from './Workspace';
import { useModalFocus } from './modal-focus';
import { terminalShortcut } from './terminal-shortcuts';
import '@xterm/xterm/css/xterm.css';
import './terminal.css';

type Info = { shell: string; directory: string };
type Output = { data: number[]; exited: boolean; exitCode: number | null; error: string | null };
type Status = 'starting' | 'running' | 'error';
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);
const DEFAULT_FONT_SIZE = 13;
const FONT_SIZE_KEY = 'fairway-terminal-font-size';

function terminalTheme(host: HTMLElement) {
  const style = getComputedStyle(host);
  const color = (name: string) => style.getPropertyValue(`--terminal-${name}`).trim();
  const foreground = color('foreground');
  return {
    background: color('background'), foreground, cursor: '#a3d9bc', selectionBackground: color('selection'),
    black: color('black'), red: '#ec9993', green: '#a0d5af', yellow: '#e6ce92', blue: '#9dbfe6', magenta: '#c4a9d9', cyan: '#92d0cc', white: foreground,
    brightBlack: color('bright-black'), brightRed: '#f5b1aa', brightGreen: '#b8e8c5', brightYellow: '#f4dfa9', brightBlue: '#b9d6f5', brightMagenta: '#dac0ef', brightCyan: '#b3e5e0', brightWhite: '#ffffff',
  };
}

function savedFontSize() {
  try {
    const value = Number(localStorage.getItem(FONT_SIZE_KEY));
    if (Number.isInteger(value) && value >= 10 && value <= 24) return value;
  } catch { /* Storage is optional; the terminal still works without it. */ }
  return DEFAULT_FONT_SIZE;
}

export type TerminalLaunch = { key: string; folder: string; label: string };
type Tab = { key: string; slot: number; folder: string | null; label?: string };
type Context = { defaultDirectory: string; folders: string[]; sites: { folder: string; label: string }[]; historySupported: boolean };
type NoticeBody = { error: boolean; text: string; warning?: boolean; action?: string };
type SlotNotice = NoticeBody | null;
type ShownNotice = (NoticeBody & { onAction?: () => void; onDismiss?: () => void }) | null;

function FixedNotice({ notice }: { notice: ShownNotice }) {
  return <div className={`terminal-notice${notice ? ` notice${notice.error ? ' error' : ''}${notice.warning ? ' terminal-notice-warning' : ''}` : ''}`}>
    <div className="terminal-notice-message">
      <div role={notice && !notice.error ? 'status' : undefined} aria-live="polite" aria-atomic="true">{notice && !notice.error ? notice.text : null}</div>
      <div role={notice?.error ? 'alert' : undefined} aria-live="assertive" aria-atomic="true">{notice?.error ? notice.text : null}</div>
    </div>
    {notice?.action && <button onClick={notice.onAction}>{notice.action}</button>}
    {notice?.onDismiss && <button aria-label="Dismiss terminal message" onClick={notice.onDismiss}>×</button>}
  </div>;
}

function savedHistory() {
  try { return localStorage.getItem('fairway-terminal-history') !== 'false'; } catch { return true; }
}

function useDismissDetails(ref: RefObject<HTMLDetailsElement | null>, active: boolean) {
  useEffect(() => {
    const details = ref.current;
    if (!details) return;
    if (!active) details.open = false;
    const dismiss = (event: PointerEvent) => {
      if (event.target instanceof Node && !details.contains(event.target)) details.open = false;
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || !details.open) return;
      event.preventDefault(); event.stopPropagation();
      details.open = false;
      details.querySelector('summary')?.focus({ preventScroll: true });
    };
    document.addEventListener('pointerdown', dismiss);
    details.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', dismiss);
      details.removeEventListener('keydown', escape);
    };
  }, [ref, active]);
}

export default function TerminalPage({ active, busy, macos, focusRequest = 0, launch = null }: { active: boolean; busy: boolean; macos: boolean; focusRequest?: number; launch?: TerminalLaunch | null }) {
  const [workspace] = useState(() => crypto.randomUUID());
  const [rememberHistory, setRememberHistory] = useState(savedHistory);
  const [tabs, setTabs] = useState<Tab[]>(() => [{ key: crypto.randomUUID(), slot: 0, folder: launch?.folder ?? null, label: launch?.label }]);
  const [selected, setSelected] = useState(0);
  const activeSlot = tabs.some(tab => tab.slot === selected) ? selected : tabs[0].slot;
  const [focusInput, setFocusInput] = useState(true);
  const newTerminalMenu = useRef<HTMLDetailsElement>(null);
  const closeBySlot = useRef<Record<number, () => void>>({});
  const externalBySlot = useRef<Record<number, () => void>>({});
  useDismissDetails(newTerminalMenu, active);
  const [context, setContext] = useState<Context>({ defaultDirectory: '', folders: [], sites: [], historySupported: false });
  const [contextError, setContextError] = useState<string | null>(null);
  const handledLaunch = useRef(launch?.key);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [statuses, setStatuses] = useState<Record<number, Status>>({ 0: 'starting' });
  const [slotNotices, setSlotNotices] = useState<Record<number, SlotNotice>>({});
  const retryBySlot = useRef<Record<number, () => void>>({});
  const clearErrorBySlot = useRef<Record<number, () => void>>({});
  const reportNotice = useCallback((slot: number, notice: SlotNotice) => {
    setSlotNotices(previous => {
      const current = previous[slot];
      if (current?.text === notice?.text && current?.error === notice?.error && current?.warning === notice?.warning && current?.action === notice?.action) return previous;
      if (!current && !notice) return previous;
      return { ...previous, [slot]: notice };
    });
  }, []);
  useEffect(() => { setFocusInput(true); }, [focusRequest]);
  const refreshContext = useCallback(async () => {
    if (!canControl) return;
    try { setContext(await invoke<Context>('terminal_context')); setContextError(null); }
    catch (reason) { setContextError(errorText(reason)); }
  }, []);
  useEffect(() => { if (active) void refreshContext(); }, [active, refreshContext]);
  const reportStatus = useCallback((slot: number, status: Status) => {
    setStatuses(previous => previous[slot] === status ? previous : { ...previous, [slot]: status });
  }, []);
  useEffect(() => {
    if (!launch || handledLaunch.current === launch.key || busy) return;
    const existing = tabs.find(tab => tab.folder === launch.folder);
    if (existing) {
      handledLaunch.current = launch.key; setSelected(existing.slot); setFocusInput(true); setLaunchError(null); return;
    }
    if (tabs.some(tab => statuses[tab.slot] === 'starting')) return;
    handledLaunch.current = launch.key;
    if (tabs.length >= 2) {
      setLaunchError('Both terminals are open. Close the extra tab, then open the site terminal again.');
      return;
    }
    const slot = tabs.some(tab => tab.slot === 0) ? 1 : 0;
    setStatuses(previous => ({ ...previous, [slot]: 'starting' }));
    setTabs(previous => [...previous, { key: crypto.randomUUID(), slot, folder: launch.folder, label: launch.label }]);
    setSelected(slot); setFocusInput(true); setLaunchError(null);
  }, [launch, busy, tabs, statuses]);
  function addTab(folder: string | null = null) {
    if (busy || tabs.length >= 2 || tabs.some(tab => statuses[tab.slot] === 'starting')) return;
    const slot = tabs.some(tab => tab.slot === 0) ? 1 : 0;
    setStatuses(previous => ({ ...previous, [slot]: 'starting' }));
    if (newTerminalMenu.current) newTerminalMenu.current.open = false;
    const next = { key: crypto.randomUUID(), slot, folder, label: context.sites.find(site => site.folder === folder)?.label };
    setLaunchError(null);
    setTabs(previous => [...previous, next]);
    setSelected(slot); setFocusInput(true);
  }
  const closeTab = useCallback((slot: number) => {
    if (slot === 0) return;
    setTabs(previous => previous.filter(tab => tab.slot !== slot));
    setSelected(previous => previous === slot ? 0 : previous);
    setFocusInput(true);
  }, []);
  const slotNotice = slotNotices[activeSlot] ?? null;
  const workspaceNotice: ShownNotice = slotNotice
    ? {
      ...slotNotice,
      onAction: slotNotice.action ? () => retryBySlot.current[activeSlot]?.() : undefined,
      onDismiss: slotNotice.error ? () => clearErrorBySlot.current[activeSlot]?.() : undefined,
    }
    : launchError ? { error: false, text: launchError, onDismiss: () => setLaunchError(null) }
    : contextError ? { error: true, text: `Could not refresh folders: ${contextError}`, action: 'Retry', onAction: () => { void refreshContext(); }, onDismiss: () => setContextError(null) }
    : null;
  const cannotAdd = busy || tabs.length >= 2 || tabs.some(tab => statuses[tab.slot] === 'starting');
  const connectionStatus = <span className={`status-label ${statuses[activeSlot] === 'running' ? 'ready' : ''}`} role="status"><span className={`status-dot ${statuses[activeSlot] === 'running' ? 'running' : ''}`} />{canControl ? { starting: 'Starting…', running: 'Connected', error: 'Disconnected' }[statuses[activeSlot] ?? 'starting'] : 'Desktop app required'}</span>;
  return <Workspace id="terminal" title="Terminal" className="terminal-workspace" actions={connectionStatus}>
    <FixedNotice notice={workspaceNotice} />
    <div className="page-heading"><div><h1>Terminal</h1><p>Local shell with PHP, Composer and the Fairway XMP CLI.</p></div>{connectionStatus}</div>
    <div className="terminal-panel">
      {canControl && <div className="terminal-header">
        <div role="tablist" aria-label="Terminals" className="terminal-tabs">{tabs.map(tab => <div className={`terminal-tab${activeSlot === tab.slot ? ' selected' : ''}`} key={tab.key}>
          <button id={`terminal-tab-${tab.slot}`} role="tab" aria-selected={activeSlot === tab.slot} aria-controls={`terminal-panel-${tab.slot}`} tabIndex={activeSlot === tab.slot ? 0 : -1} title={tab.label ?? tab.folder ?? context.defaultDirectory} onClick={() => { setSelected(tab.slot); setFocusInput(true); }} onKeyDown={event => {
            if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
            event.preventDefault();
            const index = tabs.findIndex(item => item.slot === tab.slot);
            const next = tabs[event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length];
            setFocusInput(false); setSelected(next.slot);
            document.getElementById(`terminal-tab-${next.slot}`)?.focus({ preventScroll: true });
          }}><TerminalIcon size={14} aria-hidden="true" /><span className="terminal-tab-name">Terminal {tab.slot + 1}</span>{tab.folder && <span className="terminal-tab-folder"> · {tab.label ?? tab.folder}</span>}</button>
          {tab.slot !== 0 && <button className="terminal-tab-close" aria-label={`Close terminal ${tab.slot + 1}`} disabled={statuses[tab.slot] === 'starting'} onClick={() => { setSelected(tab.slot); setFocusInput(false); closeBySlot.current[tab.slot]?.(); }}><X size={13} /></button>}
        </div>)}</div>
        <details className="terminal-new" ref={newTerminalMenu} onToggle={event => { if (event.currentTarget.open) void refreshContext(); }}>
          <summary role="button" aria-label="New terminal" aria-disabled={cannotAdd} title={tabs.length >= 2 ? 'Close the extra tab to open another terminal.' : 'New terminal'} onClick={event => { if (cannotAdd) event.preventDefault(); }}><Plus size={16} /></summary>
          <div className="terminal-popover terminal-folder-menu">
            <div className="terminal-menu-label">New terminal in</div>
            <button className="terminal-default-folder" disabled={cannotAdd || !context.defaultDirectory} onClick={() => addTab()}><span>{context.defaultDirectory || 'Loading folder…'}<small>Default location</small></span></button>
            {context.folders.map(name => <button key={name} disabled={cannotAdd} onClick={() => addTab(name)}>{name}</button>)}
            {context.sites.length > 0 && <><div className="terminal-menu-label terminal-menu-divider">Registered sites</div>{context.sites.map(site => <button key={site.folder} disabled={cannotAdd} onClick={() => addTab(site.folder)}>{site.label}</button>)}</>}
          </div>
        </details>
        <button className="terminal-external" aria-label="Open in external terminal" title="Open your system terminal in the current folder" disabled={statuses[activeSlot] !== 'running' || busy} onClick={() => externalBySlot.current[activeSlot]?.()}><ExternalLink size={14} aria-hidden="true" /><span>External terminal</span></button>
        <MaximizeWorkspace id="terminal" title="Terminal" />
      </div>}
      {tabs.map(tab => <div key={tab.key} id={`terminal-panel-${tab.slot}`} className="terminal-tab-panel" role="tabpanel" aria-labelledby={canControl ? `terminal-tab-${tab.slot}` : undefined} hidden={activeSlot !== tab.slot}>
        <TerminalSession macos={macos} active={active && activeSlot === tab.slot} focusInput={focusInput} focusRequest={focusRequest} rememberHistory={rememberHistory} setRememberHistory={checked => {
          setRememberHistory(checked);
          try { localStorage.setItem('fairway-terminal-history', String(checked)); } catch { /* Optional preference. */ }
        }} historySupported={context.historySupported} defaultDirectory={context.defaultDirectory} busy={busy} workspace={workspace} tab={tab} closeTab={closeTab} closeBySlot={closeBySlot} externalBySlot={externalBySlot} reportStatus={reportStatus} reportNotice={reportNotice} retryBySlot={retryBySlot} clearErrorBySlot={clearErrorBySlot} />
      </div>)}

    </div>
  </Workspace>;
}

function TerminalSession({ macos, active, focusInput, focusRequest, rememberHistory, setRememberHistory, historySupported, defaultDirectory, busy, workspace, tab, closeTab, closeBySlot, externalBySlot, reportStatus, reportNotice, retryBySlot, clearErrorBySlot }: {
  macos: boolean;
  active: boolean; focusInput: boolean; focusRequest: number; rememberHistory: boolean; setRememberHistory: (checked: boolean) => void; historySupported: boolean; defaultDirectory: string; busy: boolean; workspace: string; tab: Tab;
  closeTab: (slot: number) => void; closeBySlot: { current: Record<number, () => void> }; externalBySlot: { current: Record<number, () => void> }; reportStatus: (slot: number, status: Status) => void;
  reportNotice: (slot: number, notice: SlotNotice) => void;
  retryBySlot: { current: Record<number, () => void> }; clearErrorBySlot: { current: Record<number, () => void> };
}) {
  const host = useRef<HTMLDivElement>(null);
  const terminal = useRef<Terminal | null>(null);
  const fit = useRef<(() => void) | null>(null);
  const retryResize = useRef<(() => void) | null>(null);
  const search = useRef<SearchAddon | null>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const session = useRef<string | null>(null);
  const activeRef = useRef(active);
  const historyPreference = useRef(rememberHistory);
  const dialog = useRef<HTMLDivElement>(null);
  const [generation, setGeneration] = useState(0);
  const [status, setStatus] = useState<Status>('starting');
  const [info, setInfo] = useState<Info | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [resizeWarning, setResizeWarning] = useState(false);
  const [fontSize, setFontSize] = useState(savedFontSize);
  const fontSizeRef = useRef(fontSize);
  const [searchOpen, setSearchOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [matchFound, setMatchFound] = useState<boolean | null>(null);
  const [hasSelection, setHasSelection] = useState(false);
  const [copied, setCopied] = useState(false);
  const [confirm, setConfirm] = useState<'restart' | 'close' | null>(null);
  const [pendingRestart, setPendingRestart] = useState(false);
  const toolsMenu = useRef<HTMLDetailsElement>(null);
  const [contextPosition, setContextPosition] = useState<{ left: number; top: number } | null>(null);
  useDismissDetails(toolsMenu, active);
  closeBySlot.current[tab.slot] = () => {
    if (status === 'starting') return;
    if (status === 'running') setConfirm('close');
    else void endTerminal(false);
  };
  useEffect(() => {
    if (pendingRestart && !busy) { setPendingRestart(false); setGeneration(value => value + 1); }
  }, [pendingRestart, busy]);

  useEffect(() => { reportStatus(tab.slot, status); }, [tab.slot, status, reportStatus]);
  clearErrorBySlot.current[tab.slot] = () => setError(null);
  retryBySlot.current[tab.slot] = () => retryResize.current?.();
  useEffect(() => {
    const notice: SlotNotice = error ? { error: true, text: error }
      : resizeWarning && status === 'running' ? { error: false, warning: true, text: 'Terminal size could not be updated. Your commands are still running.', action: 'Retry resize' }
      : null;
    reportNotice(tab.slot, notice);
  }, [error, resizeWarning, status, tab.slot, reportNotice]);
  useEffect(() => () => reportNotice(tab.slot, null), [tab.slot, reportNotice]);
  useEffect(() => { historyPreference.current = rememberHistory; }, [rememberHistory]);

  useEffect(() => {
    const mount = host.current;
    if (!mount || !canControl) return;
    let disposed = false;
    let timer: number | undefined;
    let frame = 0;
    let failed = false;
    let receivedInput = false;
    let startedAt = 0;
    let queuedBytes = 0;
    let writing = Promise.resolve();
    let resizing = false;
    let pendingSize: { cols: number; rows: number } | null = null;
    const id = crypto.randomUUID();
    const decoder = new TextDecoder();
    const term = new Terminal({
      cursorBlink: true, fontSize: fontSizeRef.current, lineHeight: 1.25, scrollback: 3000,
      fontFamily: 'Menlo, Consolas, "Liberation Mono", monospace', screenReaderMode: true,
      theme: terminalTheme(mount),
    });
    const addon = new FitAddon();
    const searchAddon = new SearchAddon();
    term.loadAddon(addon); term.loadAddon(searchAddon); term.open(mount);
    term.textarea?.setAttribute('aria-label', 'Terminal input');
    // Repaint the existing xterm instance when Settings changes appearance.
    const appearanceObserver = new MutationObserver(() => { term.options.theme = terminalTheme(mount); });
    appearanceObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    // The mount is unpadded: FitAddon measures its full computed size. Keep
    // visual spacing on the outer host so the last row and cursor fit inside it.
    // Fit both the display and native PTY to the same supported dimensions.
    const fitToHost = () => {
      if (disposed || !host.current?.clientWidth || !host.current.clientHeight) return;
      const container = host.current.parentElement;
      if (container && !container.closest('.workspace-maximized')) {
        // Measure the actual header/toolbars, including wrapped headings. Use
        // document coordinates so ordinary page scrolling never resizes a PTY.
        const top = container.getBoundingClientRect().top + window.scrollY;
        container.style.setProperty('--terminal-top', `${top}px`);
      }
      const dimensions = addon.proposeDimensions();
      if (!dimensions || !Number.isFinite(dimensions.cols) || !Number.isFinite(dimensions.rows)) return;
      term.resize(Math.max(2, Math.min(500, dimensions.cols)), Math.max(1, Math.min(300, dimensions.rows)));
    };
    const syncSize = async () => {
      if (resizing) return;
      resizing = true;
      try {
        while (pendingSize && !disposed && session.current === id) {
          const dimensions = pendingSize; pendingSize = null;
          try {
            await invoke('terminal_resize', { id, ...dimensions });
            if (!disposed && session.current === id) setResizeWarning(false);
          } catch {
            // Layout errors must never end the shell or its running commands.
            if (!disposed && session.current === id) setResizeWarning(true);
          }
        }
      } finally { resizing = false; }
    };
    const queueSize = () => {
      if (disposed || session.current !== id) return;
      pendingSize = { cols: term.cols, rows: term.rows };
      void syncSize();
    };
    terminal.current = term; fit.current = fitToHost; search.current = searchAddon; retryResize.current = queueSize;
    setStatus('starting'); setError(null); setInfo(null); setHasSelection(false);
    setResizeWarning(false); setMatchFound(null);

    const fail = (reason: unknown) => {
      if (disposed || failed) return;
      failed = true; setError(errorText(reason)); setStatus('error');
      term.options.disableStdin = true; session.current = null;
      void invoke('terminal_close', { id }).catch(() => {});
    };
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        fitToHost();
      });
    });
    observer.observe(mount);
    const resize = term.onResize(queueSize);
    const selection = term.onSelectionChange(() => { if (!disposed) setHasSelection(term.hasSelection()); });
    const input = term.onData(data => {
      if (session.current !== id || failed) return;
      receivedInput = true;
      // Serialize keys and chunk pastes; never queue unlimited IPC writes.
      if (queuedBytes + data.length > 256 * 1024) { setError('Paste is too large. Use a smaller selection.'); return; }
      queuedBytes += data.length;
      writing = writing.then(async () => {
        for (let offset = 0; offset < data.length;) {
          let end = Math.min(offset + 1024, data.length);
          if (end < data.length && /[\uD800-\uDBFF]/.test(data[end - 1])) end--;
          if (disposed || failed || session.current !== id) break;
          await invoke('terminal_write', { id, data: data.slice(offset, end) });
          offset = end;
        }
      }).catch(reason => { if (session.current === id) fail(reason); }).finally(() => { queuedBytes -= data.length; });
    });
    term.attachCustomKeyEventHandler(event => {
      if ((event.metaKey || (event.ctrlKey && event.shiftKey)) && ['c', 'v'].includes(event.key.toLowerCase())) return false;
      if (event.ctrlKey && !event.shiftKey && event.key.toLowerCase() === 'c' && term.hasSelection()) return false;
      return true;
    });
    const poll = async () => {
      if (disposed || failed || session.current !== id) return;
      try {
        const output = await invoke<Output>('terminal_read', { id });
        if (disposed || session.current !== id) return;
        const text = decoder.decode(new Uint8Array(output.data), { stream: !output.exited });
        // Drain more only after xterm has parsed this batch, bounding its queue.
        if (text) await new Promise<void>(resolve => term.write(text, resolve));
        if (disposed || session.current !== id) return;
        if (output.error) { fail(output.error); return; }
        if (output.exited) {
          // A shell that exits during startup must offer recovery, not respawn forever.
          if (!receivedInput && Date.now() - startedAt < 1000) {
            fail(`Terminal closed during startup${output.exitCode !== null ? ` (exit ${output.exitCode})` : ''}. Try opening it again.`);
            return;
          }
          term.options.disableStdin = true; session.current = null;
          try { await invoke('terminal_close', { id }); }
          catch (reason) { fail(reason); return; }
          if (disposed) return;
          if (tab.slot === 0) { setStatus('starting'); setPendingRestart(true); }
          else closeTab(tab.slot);
        } else timer = window.setTimeout(() => { void poll(); }, output.data.length ? 10 : 70);
      } catch (reason) { if (session.current === id) fail(reason); }
    };
    // StrictMode's initial cleanup runs before this opens a native shell.
    void Promise.resolve().then(async () => {
      if (disposed) return;
      try {
        fitToHost();
        const result = await invoke<Info>('terminal_start', { request: { id, workspace, slot: tab.slot, folder: tab.folder, rememberHistory: historyPreference.current, cols: Math.max(2, term.cols), rows: Math.max(1, term.rows) } });
        if (disposed) { await invoke('terminal_close', { id }); return; }
        session.current = id; startedAt = Date.now(); setInfo(result); setStatus('running');
        fitToHost(); queueSize(); if (activeRef.current) term.focus(); void poll();
      } catch (reason) { fail(reason); }
    });
    return () => {
      disposed = true; clearTimeout(timer); cancelAnimationFrame(frame); observer.disconnect();
      appearanceObserver.disconnect();
      resize.dispose(); selection.dispose(); input.dispose(); term.dispose();
      terminal.current = null; fit.current = null; search.current = null; retryResize.current = null;
      if (session.current === id) session.current = null;
      void invoke('terminal_close', { id }).catch(() => {});
    };
  }, [generation, workspace, tab.slot, tab.folder, closeTab]);

  useEffect(() => {
    activeRef.current = active;
    if (active) {
      const frame = requestAnimationFrame(() => { fit.current?.(); if (focusInput) terminal.current?.focus(); });
      return () => cancelAnimationFrame(frame);
    }
  }, [active, focusInput, focusRequest]);

  useEffect(() => {
    fontSizeRef.current = fontSize;
    if (terminal.current) terminal.current.options.fontSize = fontSize;
    try { localStorage.setItem(FONT_SIZE_KEY, String(fontSize)); } catch { /* Optional preference. */ }
    const frame = requestAnimationFrame(() => fit.current?.());
    return () => cancelAnimationFrame(frame);
  }, [fontSize, generation]);

  useEffect(() => {
    if (!searchOpen || !active) return;
    searchInput.current?.focus();
    searchInput.current?.select();
  }, [searchOpen, active]);

  useEffect(() => {
    if (searchOpen && query) setMatchFound(search.current?.findNext(query, { incremental: true }) ?? false);
    else {
      search.current?.clearDecorations();
      terminal.current?.clearSelection();
      setMatchFound(null);
    }
  }, [query, searchOpen, generation]);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1800);
    return () => clearTimeout(timer);
  }, [copied]);

  useModalFocus(dialog, !!confirm, () => setConfirm(null));

  async function clipboard(action: 'copy' | 'paste') {
    try {
      if (action === 'copy') { await navigator.clipboard.writeText(terminal.current?.getSelection() ?? ''); setCopied(true); }
      else { const text = await navigator.clipboard.readText(); terminal.current?.paste(text); terminal.current?.focus(); }
    } catch { setError(`Could not ${action}. Use your terminal’s keyboard shortcut or clipboard permissions.`); }
  }

  externalBySlot.current[tab.slot] = () => { void openExternal(); };

  async function openExternal() {
    const id = session.current;
    if (!id) return;
    if (toolsMenu.current) toolsMenu.current.open = false;
    try { await invoke('terminal_external', { id }); }
    catch (reason) { setError(`Could not open external terminal: ${errorText(reason)}`); }
  }

  async function endTerminal(restart: boolean) {
    setConfirm(null); setError(null);
    const id = session.current; session.current = null;
    if (terminal.current) terminal.current.options.disableStdin = true;
    setStatus('starting');
    try {
      if (id) await invoke('terminal_close', { id });
      if (restart) setPendingRestart(true);
      else closeTab(tab.slot);
    } catch (reason) { setError(errorText(reason)); setStatus('error'); }
  }

  function zoom(change: number) {
    setFontSize(value => Math.max(10, Math.min(24, value + change)));
    terminal.current?.focus();
  }

  function resetZoom() {
    setFontSize(DEFAULT_FONT_SIZE);
    terminal.current?.focus();
  }

  function closeSearch() {
    setSearchOpen(false);
    terminal.current?.focus();
  }

  function findMatch(previous = false) {
    if (!query || !search.current) return;
    setMatchFound(previous ? search.current.findPrevious(query) : search.current.findNext(query));
  }

  return <section className="terminal-page" aria-label="Embedded terminal" onKeyDownCapture={event => {
    if (!active || confirm || !canControl) return;
    const shortcut = terminalShortcut(event, macos);
    if (shortcut) {
      event.preventDefault(); event.stopPropagation();
      if (shortcut === 'find') { setSearchOpen(true); searchInput.current?.focus(); searchInput.current?.select(); }
      else if (shortcut === 'reset') resetZoom();
      else zoom(shortcut === 'decrease' ? -1 : 1);
    } else if (event.key === 'Escape' && searchOpen && event.target === searchInput.current) {
      event.preventDefault(); event.stopPropagation();
      closeSearch();
    }
  }}>
    {isUiTest && <div className="preview-banner">UI test — simulated terminal. Commands do not run on your computer.</div>}
    {canControl && <details className="terminal-tools" ref={toolsMenu} onToggle={event => { if (!event.currentTarget.open) setContextPosition(null); }}>
      <summary aria-label="Terminal options" title="Terminal options" onClick={() => setContextPosition(null)}><MoreHorizontal size={18} /></summary>
      <div className="terminal-popover terminal-options" style={contextPosition ? { position: 'fixed', ...contextPosition, right: 'auto', maxWidth: 'calc(100vw - 24px)' } : undefined}>
        <div className="terminal-menu-label">{info?.shell ?? 'Local shell'}</div>
        <button onClick={() => { if (toolsMenu.current) toolsMenu.current.open = false; setSearchOpen(true); }}><Search size={14} />Find output<span className="terminal-shortcut">{macos ? '⌘ F' : 'Ctrl+Shift+F'}</span></button>
        <button onClick={() => { terminal.current?.clear(); if (toolsMenu.current) toolsMenu.current.open = false; terminal.current?.focus(); }}><Eraser size={14} />Clear output</button>
        <button disabled={!hasSelection} onClick={() => { void clipboard('copy'); }}>{copied ? <Check size={14} /> : <Copy size={14} />}Copy</button>
        <button disabled={status !== 'running'} onClick={() => { if (toolsMenu.current) toolsMenu.current.open = false; void clipboard('paste'); }}><Clipboard size={14} />Paste</button>
        <div className="terminal-zoom terminal-menu-divider" role="group" aria-label="Terminal text size">
          <span>Text size</span><button aria-label="Decrease terminal text size" disabled={fontSize <= 10} onClick={() => zoom(-1)}><Minus size={14} /></button>
          <button className="terminal-font-size" aria-label={`Reset terminal text size (${fontSize} pixels)`} title={`Reset text size (${macos ? '⌘+0' : 'Ctrl+Shift+0'})`} onClick={resetZoom}>{fontSize} px</button>
          <button aria-label="Increase terminal text size" disabled={fontSize >= 24} onClick={() => zoom(1)}><Plus size={14} /></button>
        </div>
        {historySupported && <label className="terminal-history terminal-menu-divider"><input type="checkbox" checked={rememberHistory} onChange={event => setRememberHistory(event.target.checked)} /><span>Remember commands<small>Applies to new terminals. A leading space skips saving.</small></span></label>}
        <button className="terminal-menu-divider" disabled={status === 'starting' || busy} onClick={() => { if (toolsMenu.current) toolsMenu.current.open = false; if (status === 'running') setConfirm('restart'); else void endTerminal(true); }}><RefreshCw size={14} />{status === 'running' ? 'Restart terminal' : 'Retry terminal'}</button>
        <GuideLink section="cli" className="terminal-guide" onOpen={() => { if (toolsMenu.current) toolsMenu.current.open = false; }} onError={setError}><BookOpen size={14} aria-hidden="true" />CLI guide</GuideLink>
        <div className="terminal-details"><span>Started in</span><code>{info?.directory ?? tab.label ?? tab.folder ?? defaultDirectory}</code><small>Ctrl+C interrupts a command. Type exit to finish.<br />Terminals keep running until you quit Fairway XMP.</small></div>
      </div>
    </details>}
      {searchOpen && <div id={`terminal-search-${tab.slot}`} className="terminal-search" role="search" aria-label="Terminal output">
        <input ref={searchInput} type="text" aria-label="Find in terminal" placeholder="Find in terminal" value={query} spellCheck={false} autoComplete="off" onChange={event => setQuery(event.target.value)} onKeyDown={event => {
          if (event.key === 'Enter') { event.preventDefault(); findMatch(event.shiftKey); }
        }} />
        <span className="terminal-search-result" role="status">{query && matchFound !== null ? matchFound ? 'Match found' : 'No matches' : ''}</span>
        <button className="secondary" aria-label="Previous terminal match" disabled={!query} onClick={() => findMatch(true)}><ChevronUp size={14} /></button>
        <button className="secondary" aria-label="Next terminal match" disabled={!query} onClick={() => findMatch()}><ChevronDown size={14} /></button>
        <button className="secondary" aria-label="Close terminal search" onClick={closeSearch}><X size={14} /></button>
      </div>}
      {canControl ? <div className="terminal-host" onContextMenu={event => {
        event.preventDefault();
        setContextPosition({ left: Math.max(8, Math.min(event.clientX, window.innerWidth - 306)), top: Math.max(8, Math.min(event.clientY, window.innerHeight - Math.min(460, window.innerHeight * .65) - 8)) });
        if (toolsMenu.current) { toolsMenu.current.open = true; toolsMenu.current.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus({ preventScroll: true }); }
      }}><div ref={host} className="terminal-mount" /></div> : <p className="terminal-unavailable">Open the Fairway XMP desktop app to use the terminal.</p>}
      {canControl && status === 'error' && <div className="terminal-recovery"><span>Could not open or reconnect to this terminal.</span><button className="secondary" disabled={busy} onClick={() => { void endTerminal(true); }}>Retry terminal</button></div>}
    {confirm && <div className="modal-backdrop"><div ref={dialog} className="modal terminal-confirm" role="dialog" aria-modal="true" aria-labelledby={`terminal-confirm-${tab.slot}`}><h2 id={`terminal-confirm-${tab.slot}`}>{confirm === 'restart' ? 'Restart terminal?' : 'Close terminal tab?'}</h2><p>This ends this terminal and its running commands. {confirm === 'restart' ? 'A fresh terminal will open in the same starting folder.' : 'Your first terminal keeps running.'}</p><div className="button-row"><button className="secondary" onClick={() => setConfirm(null)}>Cancel</button><button className="primary" onClick={() => { void endTerminal(confirm === 'restart'); }}>{confirm === 'restart' ? 'Restart terminal' : 'Close tab'}</button></div></div></div>}
  </section>;
}
