import { useEffect, useId, useRef, useState } from 'react';
import { ExternalLink } from 'lucide-react';
import { canControl, invoke } from './bridge';
import type { LoginItemStatus } from './types';

export default function LoginSettings({ busy }: { busy: boolean }) {
  const id = useId();
  const [status, setStatus] = useState<LoginItemStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const saving = useRef(false);
  const revision = useRef(0);

  useEffect(() => {
    let disposed = false;
    const refresh = async () => {
      if (!canControl || saving.current) return;
      const current = ++revision.current;
      try {
        const value = await invoke<LoginItemStatus>('login_item_status');
        if (!disposed && current === revision.current && !saving.current) { setStatus(value); setError(null); }
      } catch (reason) {
        if (!disposed && current === revision.current && !saving.current) setError(String(reason));
      }
    };
    void refresh();
    window.addEventListener('focus', refresh);
    return () => { disposed = true; window.removeEventListener('focus', refresh); };
  }, []);

  async function change(enabled: boolean) {
    if (saving.current) return;
    saving.current = true;
    revision.current += 1;
    setPending(true); setError(null);
    try { setStatus(await invoke<LoginItemStatus>('set_login_item', { enabled })); }
    catch (reason) {
      setError(String(reason));
      // Native registration may have partially succeeded before an error.
      try { setStatus(await invoke<LoginItemStatus>('login_item_status')); } catch { /* Keep the action error. */ }
    } finally { saving.current = false; setPending(false); }
  }

  return <section className="settings-card login-settings" aria-label="Startup">
    <label className="login-setting-row">
      <span>
        <strong id={`${id}-label`}>Launch Fairway XMP at login</strong>
        <span id={`${id}-description`} className="login-setting-description">Open Fairway XMP in the background after you sign in.</span>
      </span>
      <span className="startup-toggle">
        <input type="checkbox" role="switch" aria-labelledby={`${id}-label`} aria-describedby={`${id}-description`}
          checked={status?.registered ?? false} disabled={!canControl || !status || busy || pending}
          onChange={event => { void change(event.target.checked); }} />
        <span className="startup-track" aria-hidden="true" />
      </span>
    </label>
    {!status && canControl && !error && <p role="status">Loading login settings…</p>}
    {status?.state === 'requiresApproval' && <div className="login-approval">
      <p role="status">Allow Fairway XMP in Login Items &amp; Extensions to finish setup.</p>
      <button type="button" className="secondary" disabled={!canControl || busy || pending} onClick={() => {
        void invoke<void>('open_login_settings').catch(reason => setError(String(reason)));
      }}>Open System Settings<ExternalLink size={14} /></button>
    </div>}
    {status?.state === 'otherInstallation' && <p className="settings-hint">The login entry points to another Fairway XMP installation. Enabling this setting again updates the entry to this copy.</p>}
    {error && <p className="startup-setting-error" role="alert">{error}</p>}
  </section>;
}
