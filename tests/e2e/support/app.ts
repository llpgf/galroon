// Opens the real frontend against an isolated Core, the same way the desktop shell connects.
import type { Page } from '@playwright/test';
import { zhTW } from '../../../src/locales/zh-TW';
import type { Core } from './core';

/** Traditional Chinese UI text for a translation key (tests run in the user's zh-TW locale). */
export const BASE = `http://127.0.0.1:${process.env.GALROON_E2E_VITE_PORT || 14920}`;

export function T(key: string, vars: Record<string, string | number> = {}): string {
  let text = (zhTW as Record<string, string>)[key];
  if (text === undefined) throw new Error(`Missing zh-TW text: ${key}`);
  for (const [k, v] of Object.entries(vars)) text = text.replaceAll(`{{${k}}}`, String(v));
  return text;
}

export async function openApp(page: Page, core: Core) {
  const origin = BASE;
  const session = { ...core.session, url: origin };
  await page.addInitScript(s => {
    localStorage.setItem('galroon.language', 'zh-TW');
    (window as any).isTauri = true;
    (window as any).__TAURI_INTERNALS__ = { invoke: async (command: string) => { if (command === 'core_session') return s; throw new Error('Unavailable in browser preview: ' + command); } };
  }, session);
  // Route same-origin API calls to this test's Core.
  await page.unroute(`${origin}/api/**`).catch(() => {});
  await page.route(`${origin}/api/**`, async route => {
    const u = new URL(route.request().url());
    try { await route.fulfill({ response: await route.fetch({ url: core.url + u.pathname + u.search }) }); }
    catch { await route.abort('connectionrefused'); }
  });
  await page.goto('/');
}

export async function nav(page: Page, view: 'home' | 'library' | 'lists' | 'organize' | 'tasks' | 'sources' | 'settings') {
  await page.getByRole('navigation').getByRole('button', { name: T(view), exact: false }).first().click();
}

/** Organize → row for a resource whose path contains `text`. */
export function resourceRow(page: Page, text: string) {
  return page.locator('.organize tbody tr').filter({ hasText: text });
}

export async function openMatcher(page: Page, text: string) {
  await nav(page, 'organize');
  await page.locator('.organize-search input').fill(text);
  const row = resourceRow(page, text).first();
  const primary = row.getByRole('button', { name: T('match') });
  if (await primary.count()) await primary.click();
  else { await row.locator('summary').click(); await row.getByRole('button', { name: T('match') }).click(); }
  const drawer = page.locator('.drawer');
  await drawer.waitFor();
  return drawer;
}

/** Collection card for a work, located by its accessible "open" label (display title). */
export function card(page: Page, work: { title: string }) {
  return page.locator('.book-grid .book').filter({ has: page.getByRole('button', { name: T('openWorkNamed', { name: work.title }), exact: true }) });
}

export async function searchCollection(page: Page, text: string) {
  await nav(page, 'library');
  const box = page.getByRole('search').getByRole('textbox');
  await box.fill(text);
  await box.press('Enter');
}

export async function openWork(page: Page, work: { title: string }) {
  await card(page, work).getByRole('button', { name: T('openWorkNamed', { name: work.title }), exact: true }).click();
  await page.locator('.work-facts').waitFor();
}
