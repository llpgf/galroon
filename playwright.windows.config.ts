import { defineConfig } from '@playwright/test';

// Windows layer (W) of docs/acceptance/USER_STORIES.md: drives the installed desktop app through
// WebView2's remote debugging port. scripts/windows/acceptance.ps1 installs, launches and runs this.
export default defineConfig({
  testDir: 'tests/windows',
  outputDir: 'test-output/windows/results',
  workers: 1,
  timeout: 20 * 60_000,
  expect: { timeout: 15_000 },
  reporter: [['list']],
  use: { trace: 'retain-on-failure', screenshot: 'only-on-failure' },
});
