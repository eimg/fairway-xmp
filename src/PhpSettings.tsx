import { useEffect, useRef, useState } from 'react';
import { canControl, invoke } from './bridge';
import type { PhpSettings, PhpSettingsView, Snapshot } from './types';

interface Props {
  data: Snapshot;
  busy: boolean;
  save: (version: string, settings: PhpSettings | null) => Promise<PhpSettingsView | undefined>;
}

export default function PhpSettingsCard({ data, busy, save }: Props) {
  const versions = Object.keys(data.phpVersions).sort().reverse();
  const [version, setVersion] = useState(() => data.phpVersions[data.phpVersion] ? data.phpVersion : versions[0] ?? '8.5');
  const [loaded, setLoaded] = useState<PhpSettingsView | null>(null);
  const [draft, setDraft] = useState<PhpSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const generation = useRef(0);
  const available = canControl && data.runtimeReady && !!data.phpVersions[version];
  useEffect(() => {
    const current = ++generation.current;
    setLoaded(null); setDraft(null); setError(null);
    if (available) {
      void invoke<PhpSettingsView>('get_php_settings', { version }).then(result => {
        if (generation.current === current) { setLoaded(result); setDraft(result.settings); }
      }).catch(reason => {
        if (generation.current === current) setError(String(reason));
      });
    }
    return () => { ++generation.current; };
  }, [version, available, retry]);

  const webRunning = version === data.phpVersion && data.services.some(service => (service.id === 'nginx' || service.id === 'php') && service.running);
  const ready = !!draft && loaded?.version === version;
  const disabled = busy || !available || !ready;
  function update<K extends keyof PhpSettings>(key: K, value: PhpSettings[K]) {
    setDraft(current => current ? { ...current, [key]: value } : null);
  }
  async function submit(settings: PhpSettings | null) {
    const current = generation.current;
    const result = await save(version, settings);
    if (result && generation.current === current) { setLoaded(result); setDraft(result.settings); }
  }

  return <section className="settings-card php-settings" aria-label="PHP settings">
    <h2>PHP settings</h2>
    <p>For websites, CLI PHP, and Composer. Saved separately for each PHP version and kept across app updates.</p>
    <div className="php-settings-version"><strong>PHP {version}{version === data.phpVersion ? ' · Active' : ''}</strong>
      <details className="advanced"><summary>Choose another PHP version</summary>
        <label><span>Configure PHP version<small>Editing these settings does not switch the active PHP version.</small></span>
          <select value={version} disabled={busy || !canControl} onChange={event => setVersion(event.target.value)}>
            {versions.map(item => <option key={item} value={item}>PHP {item}{item === data.phpVersion ? ' (active)' : ''}</option>)}
          </select>
        </label>
      </details>
    </div>
    {error && <div role="alert" className="notice error"><span>{error}</span><button className="secondary" disabled={busy} onClick={() => setRetry(current => current + 1)}>Retry</button></div>}
    {available && !ready && !error && <p role="status">Loading PHP settings…</p>}
    {!available && <p className="settings-hint">PHP settings are available when the bundled runtime is ready in the desktop app.</p>}
    {ready && draft && <form onSubmit={event => { event.preventDefault(); void submit(draft); }}>
      <fieldset disabled={disabled} className="php-settings-fields">
        <div className="php-settings-grid">
          <label><span>Memory limit (MiB)<small>-1 means unlimited.</small></span><input type="number" required min={-1} max={1048576} step={1} value={draft.memoryLimitMb} onChange={event => update('memoryLimitMb', Number(event.target.value))} /></label>
          <label><span>Execution time (seconds)<small>0 means unlimited. CLI PHP starts with no time limit.</small></span><input type="number" required min={0} max={86400} step={1} value={draft.maxExecutionTime} onChange={event => update('maxExecutionTime', Number(event.target.value))} /></label>
          <label><span>Upload size per file (MiB)<small>PHP POST and Nginx limits include 8 MiB extra for form data.</small></span><input type="number" required min={1} max={16384} step={1} value={draft.uploadMaxFilesizeMb} onChange={event => update('uploadMaxFilesizeMb', Number(event.target.value))} /></label>
          <label><span>Timezone<small>Use a PHP timezone such as UTC or Asia/Yangon.</small></span><input required list="php-timezones" autoComplete="off" value={draft.timezone} onChange={event => update('timezone', event.target.value)} /><datalist id="php-timezones">{loaded.timezones.map(zone => <option key={zone} value={zone} />)}</datalist></label>
        </div>
        <label className="php-settings-toggle"><input type="checkbox" checked={draft.displayErrors} onChange={event => update('displayErrors', event.target.checked)} /><span>Display errors<small>Show PHP errors in website responses. Errors are also logged.</small></span></label>
        <label className="php-settings-toggle"><input type="checkbox" checked={draft.sessionUploadProgress} onChange={event => update('sessionUploadProgress', event.target.checked)} /><span>Session upload progress<small>Enable only for apps that do not change session settings at runtime. Regular file uploads and browser upload progress still work when this is off.</small></span></label>
      </fieldset>
      {webRunning && <p className="settings-hint">Stop Nginx and PHP from Environment before saving settings for the active version.</p>}
      {version !== data.phpVersion && <p className="settings-hint">These settings will apply when PHP {version} becomes active.</p>}
      <p className="settings-hint">{loaded.customized ? 'Custom settings saved for this version.' : 'Using defaults for this version.'}</p>
      <div className="button-row"><button className="primary" type="submit" disabled={disabled || webRunning}>Save PHP settings</button><button className="secondary" type="button" disabled={disabled || webRunning} onClick={() => { void submit(null); }}>Reset to defaults</button></div>
    </form>}
  </section>;
}
