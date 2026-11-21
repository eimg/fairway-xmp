import { defineConfig } from '@playwright/test';
import browserConfig from './playwright.config';

export default defineConfig({
  ...browserConfig,
  testMatch: ['terminal-native.spec.ts', 'data-browser-native.spec.ts', 'sqlite-browser-native.spec.ts', 'php-settings-native.spec.ts'],
  workers: 1,
  projects: [
    { name: 'native', use: { browserName: 'chromium', viewport: { width: 1080, height: 760 } } },
  ],
  // Candidate jobs already built dist. Local runs build it before previewing.
  // No Vite watcher is needed for the real Rust/PHP stdio bridge.
  webServer: [{
    command: `${process.env.FAIRWAY_NATIVE_ONLY === '1' ? '' : 'npm run build && '}npm run preview -- --port 1422 --strictPort`,
    url: 'http://127.0.0.1:1422',
    reuseExistingServer: false,
    stdout: 'pipe',
    timeout: 180000,
  }],
});
