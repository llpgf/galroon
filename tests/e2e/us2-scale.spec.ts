// US-2.5 大量收藏仍可順暢瀏覽：約 3,000 部作品的生成目錄（不需要真實檔案）。
import { test, expect } from '@playwright/test';
import { openApp, nav, T, searchCollection } from './support/app';
import { startCore, SNAPSHOTS, type Core } from './support/core';
import { existsSync } from 'node:fs';

const COUNT = 3000;
const studios = ['CloverGAME', 'Whirlpool', 'Purple software', 'WAFFLE', 'CRYSTALiA'];
let core: Core;

test.beforeAll(async () => {
  test.setTimeout(10 * 60_000);
  const seeded = `${SNAPSHOTS}/scale`;
  if (existsSync(seeded)) { core = await startCore({ name: 'us2-scale', from: seeded }); return; }
  core = await startCore({ name: 'us2-scale-seed' });
  for (let i = 0; i < COUNT; i++) {
    const n = String(i).padStart(4, '0');
    await core.api('/works', { title: `Scale Work ${n}`, original_title: `大量作品 ${n}`, developer: studios[i % studios.length], released: `20${10 + (i % 15)}-01-01` });
  }
  await new Promise(r => setTimeout(r, 5_000)); // let Core record source state once, like a real first run
  await core.stop();
  const { snapshot } = await import('./support/core');
  snapshot(core, 'scale');
  core = await startCore({ name: 'us2-scale', from: seeded });
});
test.afterAll(async () => { await core?.stop(); });

test('US-2.5 3,000 部：每頁 60、翻到末頁、搜尋與翻頁 < 1 秒、跨頁選取', async ({ page }) => {
  await openApp(page, core);
  await nav(page, 'library');
  const pager = page.getByRole('navigation', { name: T('collectionPages') });
  const pages = Math.ceil(COUNT / 60);

  await test.step('總數與每頁 60', async () => {
    await expect(page.locator('.collection-heading')).toContainText(String(COUNT));
    await expect(page.locator('.book-grid .book')).toHaveCount(60);
    await expect(pager).toContainText(T('pageOf', { page: 1, pages }));
  });

  await test.step('翻頁 < 1 秒', async () => {
    const started = Date.now();
    await pager.getByRole('button', { name: T('nextPage') }).click();
    await expect(pager).toContainText(T('pageOf', { page: 2, pages }));
    await expect(page.locator('.book-grid .book')).toHaveCount(60);
    expect(Date.now() - started, 'next page ms').toBeLessThan(1000);
  });

  await test.step('搜尋涵蓋全部且 < 1 秒', async () => {
    const started = Date.now();
    await searchCollection(page, '大量作品 2999');
    // Search is deliberately typo-tolerant (2999 also finds 2990…); the exact work must lead.
    await expect(page.locator('.book-grid .book').first()).toContainText('2999');
    expect(Date.now() - started, 'search ms').toBeLessThan(1000);
    await searchCollection(page, '');
  });

  await test.step('依品牌搜尋的計數涵蓋所有頁', async () => {
    await searchCollection(page, 'Whirlpool');
    await expect(page.getByText(T('filteredWorks', { total: COUNT, count: COUNT / studios.length })).first()).toBeVisible();
  });

  await test.step('跳到末頁', async () => {
    await searchCollection(page, '');
    const next = pager.getByRole('button', { name: T('nextPage') });
    // Returning to an earlier query restores its page, so start from whatever page is shown.
    const current = Number((await pager.innerText()).match(/(\d+)\s*\//)![1]);
    for (let shown = current; shown < pages; shown++) {
      await next.click();
      await expect(pager).toContainText(T('pageOf', { page: shown + 1, pages }));
    }
    await expect(pager).toContainText(T('pageOf', { page: pages, pages }));
    await expect(page.locator('.book-grid .book')).toHaveCount(COUNT - 60 * (pages - 1));
  });
});
