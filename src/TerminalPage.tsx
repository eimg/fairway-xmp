import { useCallback, useEffect, useRef, useState } from 'react';
import { Check, ChevronDown, ChevronUp, Clipboard, Copy, Eraser, Maximize2, Minimize2, Minus, Plus, RefreshCw, Search, Square, Terminal as TerminalIcon, X } from 'lucide-react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { canControl, invoke, isUiTest } from './bridge';
import '@xterm/xterm/css/xterm.css';
import './terminal.css';

type Info = { shell: string; directory: string };
type Output = { data: number[]; exited: boolean; exitCode: number | null; error: string | null };
type Status = 'starting' | 'running' | 'ended' | 'error';
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);
const DEFAULT_FONT_SIZE = 13;
const FONT_SIZE_KEY = 'fairway-terminal-font-size';

function savedFontSize() {
  try {
    const value = Number(localStorage.getItem(FONT_SIZE_KEY));
    if (Number.isInteger(value) && value >= 10 && value <= 24) return value;
  } catch { /* Storage is optional; the terminal still works without it. */ }
  return DEFAULT_FONT_SIZE;
}

type Tab = { key: string; slot: number; folder: string | null };
type Context = { folders: string[]; historySupported: boolean };

function savedHistory() {
  try { return localStorage.getItem('fairway-terminal-history') !== 'false'; } catch { return true; }
}

export default function TerminalPage({ active, busy, focusRequest = 0 }: { active: boolean; busy: boolean; focusRequest?: number }) {
  const [workspace] = useState(() => crypto.randomUUID());
  const [rememberHistory, setRememberHistory] = useState(savedHistory);
  const [tabs, setTabs] = useState<Tab[]>(() => [{ key: crypto.randomUUID(), slot: 0, folder: null }]);
  const [selected, setSelected] = useState(0);
  const activeSlot = tabs.some(tab => tab.slot === selected) ? selected : tabs[0].slot;
  const [expanded, setExpanded] = useState(false);
  const [focusInput, setFocusInput] = useState(true);
  const [folder, setFolder] = useState('');
  const [context, setContext] = useState<Context>({ folders: [], historySupported: false });
  const [contextError, setContextError] = useState<string | null>(null);
  const [statuses, setStatuses] = useState<Record<number, Status>>({ 0: 'starting' });
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
  function addTab() {
    if (busy || tabs.length >= 2 || tabs.some(tab => statuses[tab.slot] === 'starting')) return;
    const slot = tabs.some(tab => tab.slot === 0) ? 1 : 0;
    setStatuses(previous => ({ ...previous, [slot]: 'starting' }));
    const next = { key: crypto.randomUUID(), slot, folder: folder || null };
    setTabs(previous => [...previous, next]);
    setSelected(slot); setFocusInput(true);
  }
  function closeTab(slot: number) {
    const remaining = tabs.filter(tab => tab.slot !== slot);
    // A second close may complete before this one. Never resurrect its old tab
    // or leave the workspace without a visible session/recovery action.
    setTabs(previous => previous.length > 1 ? previous.filter(tab => tab.slot !== slot) : previous);
    setSelected(remaining[0]?.slot ?? slot); setFocusInput(true);
  }
  return <div className={`terminal-workspace${expanded ? ' terminal-expanded' : ''}`}>
    <div className="page-heading"><div><h1>Terminal</h1><p>PHP and Composer, ready in your coursework folder.</p></div><span className={`status-label ${statuses[activeSlot] === 'running' ? 'ready' : ''}`} role="status"><span className={`status-dot ${statuses[activeSlot] === 'running' ? 'running' : ''}`} />{canControl ? { starting: 'Starting…', running: 'Connected', ended: 'Session ended', error: 'Disconnected' }[statuses[activeSlot] ?? 'starting'] : 'Desktop app required'}</span></div>
    {canControl && <div className="terminal-workspace-controls">
      <div role="tablist" aria-label="Terminal sessions" className="terminal-tabs">{tabs.map(tab => <button key={tab.key} id={`terminal-tab-${tab.slot}`} role="tab" aria-selected={activeSlot === tab.slot} aria-controls={`terminal-panel-${tab.slot}`} tabIndex={activeSlot === tab.slot ? 0 : -1} title={tab.folder ?? 'Coursework folder'} onClick={() => { setSelected(tab.slot); setFocusInput(true); }} onKeyDown={event => {
        if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
        event.preventDefault();
        const index = tabs.findIndex(item => item.slot === tab.slot);
        const next = tabs[event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length];
        setFocusInput(false); setSelected(next.slot);
        document.getElementById(`terminal-tab-${next.slot}`)?.focus();
      }}>Terminal {tab.slot + 1}{tab.folder && <span> · {tab.folder}</span>}</button>)}</div>
      <div className="terminal-new-session"><label>Start in<select aria-label="New terminal folder" value={folder} onFocus={() => { void refreshContext(); }} onChange={event => setFolder(event.target.value)}><option value="">Coursework folder</option>{folder && !context.folders.includes(folder) && <option value={folder}>{folder} (unavailable)</option>}{context.folders.map(name => <option key={name} value={name}>{name}</option>)}</select></label>
        <button className="secondary" disabled={busy || tabs.length >= 2 || tabs.some(tab => statuses[tab.slot] === 'starting')} title={tabs.length >= 2 ? 'Close a tab to open another terminal.' : 'Open a separate shell without interrupting this one.'} onClick={addTab}><Plus size={14} />New terminal</button>
      </div>
      {context.historySupported && <label className="terminal-history"><input type="checkbox" checked={rememberHistory} onChange={event => {
        const checked = event.target.checked; setRememberHistory(checked);
        try { localStorage.setItem('fairway-terminal-history', String(checked)); } catch { /* Optional preference. */ }
      }} />Remember commands in new sessions<span title="Commands are saved privately on this device, with about 1,000 entries retained. Start a command with a space to omit it. Output and password prompts are not recorded.">Local history · leading space skips saving</span></label>}
      {contextError && <p className="terminal-context-error" role="status">Could not refresh folders: {contextError} <button className="text-link" onClick={() => { void refreshContext(); }}>Retry</button></p>}
    </div>}
    {tabs.map(tab => <div key={tab.key} id={`terminal-panel-${tab.slot}`} className="terminal-tab-panel" role="tabpanel" aria-labelledby={canControl ? `terminal-tab-${tab.slot}` : undefined} hidden={activeSlot !== tab.slot}>
      <TerminalSession active={active && activeSlot === tab.slot} focusInput={focusInput} focusRequest={focusRequest} rememberHistory={rememberHistory} busy={busy} workspace={workspace} tab={tab} expanded={expanded} toggleExpanded={() => setExpanded(value => !value)} canClose={tabs.length > 1} closeTab={() => closeTab(tab.slot)} reportStatus={reportStatus} />
    </div>)}
  </div>;
}

function TerminalSession({ active, focusInput, focusRequest, rememberHistory, busy, workspace, tab, expanded, toggleExpanded, canClose, closeTab, reportStatus }: {
  active: boolean; focusInput: boolean; focusRequest: number; rememberHistory: boolean; busy: boolean; workspace: string; tab: Tab; expanded: boolean;
  toggleExpanded: () => void; canClose: boolean; closeTab: () => void; reportStatus: (slot: number, status: Status) => void;
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
  const [confirm, setConfirm] = useState<'restart' | 'stop' | 'close' | null>(null);

  useEffect(() => { reportStatus(tab.slot, status); }, [tab.slot, status, reportStatus]);
  useEffect(() => { historyPreference.current = rememberHistory; }, [rememberHistory]);

  useEffect(() => {
    if (!host.current || !canControl) return;
    let disposed = false;
    let timer: number | undefined;
    let frame = 0;
    let failed = false;
    let queuedBytes = 0;
    let writing = Promise.resolve();
    let resizing = false;
    let pendingSize: { cols: number; rows: number } | null = null;
    const id = crypto.randomUUID();
    const decoder = new TextDecoder();
    const term = new Terminal({
      cursorBlink: true, fontSize: fontSizeRef.current, lineHeight: 1.25, scrollback: 3000,
      fontFamily: 'Menlo, Consolas, "Liberation Mono", monospace', screenReaderMode: true,
      theme: { background: '#17251f', foreground: '#e0ebe4', cursor: '#a3d9bc', selectionBackground: '#52796280',
        black: '#263b30', red: '#ec9993', green: '#a0d5af', yellow: '#e6ce92', blue: '#9dbfe6', magenta: '#c4a9d9', cyan: '#92d0cc', white: '#e0ebe4',
        brightBlack: '#8b9e91', brightRed: '#f5b1aa', brightGreen: '#b8e8c5', brightYellow: '#f4dfa9', brightBlue: '#b9d6f5', brightMagenta: '#dac0ef', brightCyan: '#b3e5e0', brightWhite: '#ffffff' },
    });
    const addon = new FitAddon();
    const searchAddon = new SearchAddon();
    term.loadAddon(addon); term.loadAddon(searchAddon); term.open(host.current);
    term.textarea?.setAttribute('aria-label', 'Terminal input');
    // Fit both the display and native PTY to the same supported dimensions.
    const fitToHost = () => {
      if (disposed || !host.current?.clientWidth || !host.current.clientHeight) return;
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
    observer.observe(host.current);
    const resize = term.onResize(queueSize);
    const selection = term.onSelectionChange(() => { if (!disposed) setHasSelection(term.hasSelection()); });
    const input = term.onData(data => {
      if (session.current !== id || failed) return;
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
          term.writeln(`\r\n[Session ended${output.exitCode !== null ? ` · exit ${output.exitCode}` : ''}]`);
          term.options.disableStdin = true; session.current = null; setStatus('ended');
          void invoke('terminal_close', { id }).catch(() => {});
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
        session.current = id; setInfo(result); setStatus('running');
        fitToHost(); queueSize(); if (activeRef.current) term.focus(); void poll();
      } catch (reason) { fail(reason); }
    });
    return () => {
      disposed = true; clearTimeout(timer); cancelAnimationFrame(frame); observer.disconnect();
      resize.dispose(); selection.dispose(); input.dispose(); term.dispose();
      terminal.current = null; fit.current = null; search.current = null; retryResize.current = null;
      if (session.current === id) session.current = null;
      void invoke('terminal_close', { id }).catch(() => {});
    };
  }, [generation, workspace, tab.slot, tab.folder]);

  useEffect(() => {
    activeRef.current = active;
    if (active) {
      const frame = requestAnimationFrame(() => { fit.current?.(); if (focusInput) terminal.current?.focus(); });
      return () => cancelAnimationFrame(frame);
    }
  }, [active, expanded, focusInput, focusRequest]);

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
  }, [searchOpen, active, expanded]);

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

  useEffect(() => {
    if (!confirm) return;
    const previous = document.activeElement as HTMLElement | null;
    dialog.current?.querySelector('button')?.focus();
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); setConfirm(null); }
      if (event.key !== 'Tab') return;
      const buttons = dialog.current?.querySelectorAll<HTMLButtonElement>('button');
      if (!buttons?.length) return;
      const first = buttons[0], last = buttons[buttons.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    };
    document.addEventListener('keydown', key);
    return () => { document.removeEventListener('keydown', key); previous?.focus(); };
  }, [confirm]);

  async function clipboard(action: 'copy' | 'paste') {
    try {
      if (action === 'copy') { await navigator.clipboard.writeText(terminal.current?.getSelection() ?? ''); setCopied(true); }
      else { const text = await navigator.clipboard.readText(); terminal.current?.paste(text); terminal.current?.focus(); }
    } catch { setError(`Could not ${action}. Use your terminal’s keyboard shortcut or clipboard permissions.`); }
  }

  async function endSession(restart: boolean, remove = false) {
    setConfirm(null); setError(null);
    const id = session.current; session.current = null;
    if (terminal.current) terminal.current.options.disableStdin = true;
    setStatus(restart ? 'starting' : 'ended');
    try {
      if (id) await invoke('terminal_close', { id });
      if (remove) closeTab();
      else if (restart) setGeneration(value => value + 1);
      else terminal.current?.writeln('\r\n[Session ended]');
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
    const modifier = event.metaKey || event.ctrlKey;
    if (modifier && ['f', '+', '=', '-', '0'].includes(event.key.toLowerCase())) {
      event.preventDefault(); event.stopPropagation();
      if (event.key.toLowerCase() === 'f') { setSearchOpen(true); searchInput.current?.focus(); searchInput.current?.select(); }
      else if (event.key === '0') resetZoom();
      else zoom(event.key === '-' ? -1 : 1);
    } else if (event.key === 'Escape' && searchOpen && event.target === searchInput.current) {
      event.preventDefault(); event.stopPropagation();
      closeSearch();
    }
  }}>
    {isUiTest && <div className="preview-banner">UI test — simulated terminal. Commands do not run on your computer.</div>}
    {error && <div className="notice error" role="alert">{error}</div>}
    {resizeWarning && status === 'running' && <div className="terminal-resize-warning" role="status"><span>Terminal size could not be updated. Your commands are still running.</span><button className="text-link" onClick={() => retryResize.current?.()}>Retry resize</button></div>}
    <div className="terminal-panel">
      <div className="terminal-toolbar">
        <div className="terminal-toolbar-heading"><span className="terminal-shell"><TerminalIcon size={16} />{info?.shell ?? 'Local shell'}</span><div className="terminal-session-actions">
          <button className="secondary" disabled={!canControl || status === 'starting' || busy} onClick={() => { if (status === 'running') setConfirm('restart'); else void endSession(true); }}><RefreshCw size={14} />{status === 'running' ? 'Restart' : 'New session'}</button>
          <button className="secondary" disabled={status !== 'running' || !canControl} onClick={() => setConfirm('stop')}><Square size={13} />End session</button>
          {canClose && <button className="secondary" onClick={() => { if (status === 'running') setConfirm('close'); else void endSession(false, true); }} disabled={status === 'starting'}><X size={14} />Close tab</button>}
        </div></div>
        <div className="terminal-actions">
        <button className="secondary" disabled={!canControl} onClick={() => { terminal.current?.clear(); terminal.current?.focus(); }}><Eraser size={14} />Clear</button>
        <button className="secondary" disabled={!hasSelection} onClick={() => { void clipboard('copy'); }}>{copied ? <Check size={14} /> : <Copy size={14} />}Copy</button>
        <button className="secondary" disabled={status !== 'running' || !canControl} onClick={() => { void clipboard('paste'); }}><Clipboard size={14} />Paste</button>
        <button className="secondary" disabled={!canControl} title="Find output (Ctrl/Cmd+F)" aria-expanded={searchOpen} aria-controls={`terminal-search-${tab.slot}`} onClick={() => { setSearchOpen(true); searchInput.current?.focus(); searchInput.current?.select(); }}><Search size={14} />Find</button>
        <div className="terminal-zoom" role="group" aria-label="Terminal text size">
          <button className="secondary" aria-label="Decrease terminal text size" disabled={!canControl || fontSize <= 10} onClick={() => zoom(-1)}><Minus size={14} /></button>
          <button className="secondary terminal-font-size" aria-label={`Reset terminal text size (${fontSize} pixels)`} disabled={!canControl} title="Reset text size (Ctrl/Cmd+0)" onClick={resetZoom}>{fontSize} px</button>
          <button className="secondary" aria-label="Increase terminal text size" disabled={!canControl || fontSize >= 24} onClick={() => zoom(1)}><Plus size={14} /></button>
        </div>
        <button className="secondary" aria-label={expanded ? 'Restore terminal size' : 'Expand terminal'} aria-pressed={expanded} disabled={!canControl} onClick={toggleExpanded}>{expanded ? <Minimize2 size={14} /> : <Maximize2 size={14} />}</button>
      </div></div>
      {searchOpen && <div id={`terminal-search-${tab.slot}`} className="terminal-search" role="search" aria-label="Terminal output">
        <input ref={searchInput} type="text" aria-label="Find in terminal" placeholder="Find in terminal" value={query} spellCheck={false} autoComplete="off" onChange={event => setQuery(event.target.value)} onKeyDown={event => {
          if (event.key === 'Enter') { event.preventDefault(); findMatch(event.shiftKey); }
        }} />
        <span className="terminal-search-result" role="status">{query && matchFound !== null ? matchFound ? 'Match found' : 'No matches' : ''}</span>
        <button className="secondary" aria-label="Previous terminal match" disabled={!query} onClick={() => findMatch(true)}><ChevronUp size={14} /></button>
        <button className="secondary" aria-label="Next terminal match" disabled={!query} onClick={() => findMatch()}><ChevronDown size={14} /></button>
        <button className="secondary" aria-label="Close terminal search" onClick={closeSearch}><X size={14} /></button>
      </div>}
      {canControl ? <div ref={host} className="terminal-host" /> : <p className="terminal-unavailable">Open the Fairway XMP desktop app to use the terminal.</p>}
      <div className="terminal-directory"><span>Started in</span><code title={info?.directory}>{info?.directory ?? 'Your coursework folder'}</code></div>
    </div>
    <p className="terminal-hint">Ctrl+C interrupts a command. Switching pages or closing the window keeps this session running. Quitting Fairway XMP ends it.</p>
    {confirm && <div className="modal-backdrop"><div ref={dialog} className="modal terminal-confirm" role="dialog" aria-modal="true" aria-labelledby={`terminal-confirm-${tab.slot}`}><h2 id={`terminal-confirm-${tab.slot}`}>{confirm === 'restart' ? 'Restart terminal?' : confirm === 'close' ? 'Close terminal tab?' : 'End terminal session?'}</h2><p>This ends this shell and its running commands. {confirm === 'restart' ? 'A new shell will open in the same starting folder.' : confirm === 'close' ? 'Your other terminal keeps running.' : 'You can start a new session afterward.'}</p><div className="button-row"><button className="secondary" onClick={() => setConfirm(null)}>Cancel</button><button className="primary" onClick={() => { void endSession(confirm === 'restart', confirm === 'close'); }}>{confirm === 'restart' ? 'Restart terminal' : confirm === 'close' ? 'Close tab' : 'End session'}</button></div></div></div>}
  </section>;
}
