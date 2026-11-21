import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/ui',
  // This is a safety net for frontend-only failures. Native wiring has its own
  // config; appearance and window-size review belong to human visual testing.
  testMatch: ['browser-smoke.spec.ts', 'frontend-recovery.spec.ts', 'security-boundaries.spec.ts'],
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  failOnFlakyTests: !!process.env.CI,
  workers: process.env.CI ? 2 : undefined,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: 'http://127.0.0.1:1421',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    { name: 'chromium', use: { browserName: 'chromium', viewport: { width: 1080, height: 760 } } },
    {
      name: 'webkit',
      testMatch: ['frontend-recovery.spec.ts', 'security-boundaries.spec.ts'],
      grep: /failed lazy terminal chunk|production CSP isolates/,
      use: { browserName: 'webkit', viewport: { width: 1080, height: 760 } },
    },
  ],
  webServer: [
    { command: 'npm run dev:ui -- --port 1421 --strictPort', url: 'http://127.0.0.1:1421', reuseExistingServer: false, stdout: 'pipe', timeout: 180000 },
    { command: 'npm run build && npm run preview -- --port 1422 --strictPort', url: 'http://127.0.0.1:1422', reuseExistingServer: false, stdout: 'pipe', timeout: 180000 },
  ],
});
