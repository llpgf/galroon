// US-2.1 用日文原名找到作品；US-2.2 用品牌或人物找作品；US-2.3 作品頁必要資訊。
import { test, expect } from '@playwright/test';
import { openApp, nav, T, card, searchCollection, openWork } from './support/app';
import { startCore, SNAPSHOTS, type Core } from './support/core';
import { catalog, type Work } from './support/golden';

let core: Core, works: Work[];
test.beforeAll(async () => { core = await startCore({ name: 'us2-browse', from: `${SNAPSHOTS}/intake` }); works = (await catalog(core)).works; });
test.afterAll(async () => { await core?.stop(); });
const byVndb = (id: string) => works.find(w => w.vndb_id === id)!;
const results = (page: import('@playwright/test').Page) => page.locator('.book-grid .book');

test.describe('US-2.1 用日文原名搜尋', () => {
  for (const [query, vndb, why] of [
    ['マガルミナ', 'v62721', '完整原名'],
    ['やりなおし', 'v59027', '部分字串'],
    ['猫忍えくすはーとＳＰＩＮ！', 'v58009', '全形英數與符號'],
    ['猫忍えくすはーとspin', 'v58009', '大小寫'],
    ['まがるみな', 'v62721', '平假名查片假名'],
    ['ヤリナオシ', 'v59027', '片假名查平假名'],
  ] as const) {
    test(`${why}：「${query}」找到作品`, async ({ page }) => {
      await openApp(page, core);
      await searchCollection(page, query);
      await expect(card(page, byVndb(vndb))).toBeVisible();
    });
  }

  test('找不到時顯示提示與清除搜尋，不是空白', async ({ page }) => {
    await openApp(page, core);
    await searchCollection(page, 'zzzz不存在的作品zzzz');
    await expect(results(page)).toHaveCount(0);
    await expect(page.getByText(T('noResults')).or(page.locator('.library-results .empty'))).toBeVisible();
    await page.getByRole('button', { name: T('clearSearch') }).click();
    await expect(results(page).first()).toBeVisible();
  });
});

test.describe('US-2.2 用品牌或人物找作品', () => {
  test('從作品頁點 CloverGAME → 列出收藏中 4 部 CloverGAME 作品', async ({ page }) => {
    await openApp(page, core);
    await nav(page, 'library');
    await openWork(page, byVndb('v59027'));
    await page.locator('.work-companies').getByRole('button', { name: /CloverGAME/i }).click();
    // やりなおしクランクイン, ダウニャーさんと飼い主くん, 飛べない蝶のバレンタイン, メイドちゃんは迷途ちゅう (VJ01004242)
    const owned = ['v59027', 'v60462', 'v62758', 'v55899'].map(byVndb);
    for (const w of owned) await expect(page.getByText(w.title, { exact: false }).first(), w.title).toBeVisible({ timeout: 60_000 });
    await expect(page.getByText(T('inCollection')).first()).toBeVisible();
  });

  test('從角色的聲優點進人物頁 → 顯示其作品並標出哪些在收藏中', async ({ page }) => {
    await openApp(page, core);
    await nav(page, 'library');
    await openWork(page, byVndb('v59027'));
    await page.getByRole('button', { name: /Kamiyama Midori/ }).first().click({ timeout: 60_000 });
    await expect(page.getByRole('heading', { name: /Kamiyama Midori/ }).first()).toBeVisible({ timeout: 60_000 });
    await expect(page.getByText(/やりなおし|Yarinaoshi/).first()).toBeVisible({ timeout: 60_000 });
    await expect(page.getByText(T('inCollection')).first()).toBeVisible();
  });
});

test('US-2.3 每部已配作品頁：封面、標題、品牌、發售日、本機路徑、角色/製作人員/關聯', async ({ page }) => {
  test.setTimeout(30 * 60_000);
  await openApp(page, core);
  const { resources } = await catalog(core);
  const problems: string[] = [];
  for (const w of works.filter(w => w.vndb_id)) {
    await nav(page, 'library');
    await searchCollection(page, '');
    await openWork(page, w);
    const facts = page.locator('.work-facts');
    const check = async (what: string, fn: () => Promise<unknown>) => { try { await fn(); } catch (e) { problems.push(`${w.vndb_id} ${w.title}: ${what} — ${String(e).split('\n')[0]}`); } };
    await check('title', () => expect(page.locator('h1').first()).toHaveText(w.title));
    await check('brand', () => expect(facts.locator('.work-companies')).not.toHaveText(T('unknown'), { timeout: 30_000 }));
    await check('released', () => expect(facts).toContainText(/\d{4}-\d{2}/));
    await check('cover', () => expect.poll(() => page.locator('.work-hero img, .detail img').first().evaluate(i => (i as HTMLImageElement).naturalWidth), { timeout: 30_000 }).toBeGreaterThan(0));
    for (const r of resources.filter(r => r.work_id === w.id)) await check(`path ${r.path}`, () => expect(page.getByText(r.path, { exact: false }).first()).toBeVisible());
    for (const section of ['characters', 'staff', 'related'] as const) {
      await check(section, () => expect(page.getByRole('heading', { name: T(section), exact: true }).first()).toBeVisible({ timeout: 30_000 }));
    }
    await check('no error alert', () => expect(page.locator('main [role=alert]')).toHaveCount(0));
    await page.getByRole('button', { name: T('back') }).first().click().catch(() => {});
  }
  console.log(problems.join('\n'));
  expect(problems).toEqual([]);
});
