import { defineConfig } from '@playwright/test';
import base from '../playwright.config';
const port = Number(process.env.FAIRWAY_PG_UI_PORT ?? '1421');
const url = `http://127.0.0.1:${port}`;
// Focused headless fixture suite. Avoid rebuilding unrelated production assets.
export default defineConfig({
  ...base, use: { ...base.use, baseURL: url }, testDir: './ui', testMatch: 'postgresql-workspace.spec.ts', workers: 2,
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report/postgresql' }]],
  webServer: [{ command: `npm run dev:ui -- --port ${port} --strictPort`, url, reuseExistingServer: false, stdout: 'pipe', timeout: 180000 }],
});
