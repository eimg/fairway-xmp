import { useState } from 'react';
import { defaultPlatform, scenarios, select } from './backend';
import type { Platform, Scenario } from './backend';

export default function Toolbar({ busy }: { busy: boolean }) {
  const [scenario, setScenario] = useState<Scenario>('Stopped');
  const [platform, setPlatform] = useState<Platform>(defaultPlatform);
  return <section className="ui-test-toolbar" aria-label="UI test controls">
    <strong>UI test · Simulated data</strong>
    <span>No real services, files, or updates are changed. Reload resets the test.</span>
    <label>Scenario<select aria-label="Scenario" disabled={busy} value={scenario} onChange={event => {
      const value = event.target.value as Scenario; setScenario(value); select(value, platform);
    }}>{scenarios.map(value => <option key={value}>{value}</option>)}</select></label>
    <label>Example paths<select aria-label="Example paths" disabled={busy} value={platform} onChange={event => {
      const value = event.target.value as Platform; setPlatform(value); select(scenario, value);
    }}><option>Windows</option><option>macOS</option></select></label>
    <button disabled={busy} className="secondary" onClick={() => window.location.reload()}>Reset test</button>
  </section>;
}
