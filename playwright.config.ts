import { defineConfig } from '@playwright/test';

const nativeOnly = process.env.FAIRWAY_NATIVE_ONLY === '1';

export default defineConfig({
  testDir: './tests/ui',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: 'http://127.0.0.1:1421',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    { name: 'chromium', use: { browserName: 'chromium', viewport: { width: 1080, height: 760 } } },
    { name: 'chromium-minimum', use: { browserName: 'chromium', viewport: { width: 820, height: 650 } } },
    { name: 'webkit', use: { browserName: 'webkit', viewport: { width: 1080, height: 760 } } },
    { name: 'webkit-minimum', use: { browserName: 'webkit', viewport: { width: 820, height: 650 } } },
  ],
  // Candidate jobs already built dist. Exercise those production assets without
  // starting a dev watcher over the staged runtime and Rust build directories.
  webServer: [
    ...(!nativeOnly ? [{ command: 'npm run dev:ui -- --port 1421', url: 'http://127.0.0.1:1421', reuseExistingServer: false, stdout: 'pipe' as const, timeout: 180000 }] : []),
    { command: `${nativeOnly ? '' : 'npm run build && '}npm run preview -- --port 1422 --strictPort`, url: 'http://127.0.0.1:1422', reuseExistingServer: false, stdout: 'pipe', timeout: 180000 },
  ],
});
