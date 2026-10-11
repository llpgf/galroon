import { defineConfig } from '@playwright/test';

// End-to-end acceptance for docs/acceptance/USER_STORIES.md.
// Each spec starts its own isolated Core (ui_core example) and drives the real frontend served by Vite.
const VITE_PORT = Number(process.env.GALROON_E2E_VITE_PORT || 14920);

export default defineConfig({
  testDir: 'tests/e2e',
  outputDir: 'test-output/e2e/results',
  workers: 1,
  timeout: 15 * 60_000,
  expect: { timeout: 15_000 },
  reporter: [['list'], ['html', { outputFolder: 'test-output/e2e/report', open: 'never' }]],
  use: { baseURL: `http://127.0.0.1:${VITE_PORT}`, locale: 'zh-TW', trace: 'retain-on-failure', screenshot: 'only-on-failure' },
  webServer: { command: `npx vite --host 127.0.0.1 --port ${VITE_PORT} --strictPort`, url: `http://127.0.0.1:${VITE_PORT}`, reuseExistingServer: true },
  projects: [
    { name: 'intake', testMatch: /us1-intake\.spec\.ts/ },
    { name: 'stories', testIgnore: /us1-intake\.spec\.ts/, dependencies: ['intake'] },
  ],
});
