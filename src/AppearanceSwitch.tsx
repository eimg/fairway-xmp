import { useState } from 'react';
import { Moon, Sun } from 'lucide-react';

type Appearance = 'light' | 'dark';
const appearanceKey = 'fairway.appearance';
const root = document.documentElement;

// Apply the stored choice before React mounts the workspace.
try {
  if (localStorage.getItem(appearanceKey) === 'dark') root.dataset.theme = 'dark';
} catch { /* Light is the default if storage is unavailable. */ }

export default function AppearanceSwitch() {
  const [appearance, setAppearance] = useState<Appearance>(() => root.dataset.theme === 'dark' ? 'dark' : 'light');

  function choose(next: Appearance) {
    root.dataset.theme = next;
    setAppearance(next);
    try { localStorage.setItem(appearanceKey, next); }
    catch { /* The choice still applies to this window. */ }
  }

  return <div className="appearance-switch" role="group" aria-label="Appearance">
    <button type="button" aria-pressed={appearance === 'light'} onClick={() => choose('light')}><Sun size={14} aria-hidden="true" />Light</button>
    <button type="button" aria-pressed={appearance === 'dark'} onClick={() => choose('dark')}><Moon size={14} aria-hidden="true" />Dark</button>
  </div>;
}
