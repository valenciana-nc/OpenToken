import { defineConfig } from '@playwright/test';
export default defineConfig({ workers: 2, testDir: './tests', use: { baseURL: 'http://127.0.0.1:1420', viewport: { width: 454, height: 760 }, screenshot: 'only-on-failure', browserName: 'chromium', channel: 'msedge' }, webServer: { command: 'npm run dev', url: 'http://127.0.0.1:1420', reuseExistingServer: !process.env.CI }, reporter: 'list' });
