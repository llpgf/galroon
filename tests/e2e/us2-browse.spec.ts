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
    for (const w of owned) await expect(page.locator(`text=${w.original_title}`).locator('visible=true').first(), w.title).toBeVisible({ timeout: 60_000 });
    // Owned works open locally; the others only link to VNDB.
    await expect(page.getByRole('button', { name: T('viewWork') }).locator('visible=true')).toHaveCount(owned.length);
    await expect(page.getByRole('link', { name: new RegExp(T('viewAtVndb')) }).locator('visible=true').first()).toBeVisible();
  });

  test('從角色的聲優點進人物頁 → 顯示其作品並標出哪些在收藏中', async ({ page }) => {
    await openApp(page, core);
    await nav(page, 'library');
    await openWork(page, byVndb('v59027'));
    await page.getByRole('button', { name: /Kamiyama Midori/ }).first().click({ timeout: 60_000 });
    await expect(page.getByRole('heading', { name: /Kamiyama Midori/ }).first()).toBeVisible({ timeout: 60_000 });
    await expect(page.getByText('やりなおしクランクイン').locator('visible=true').first()).toBeVisible({ timeout: 60_000 });
    await expect(page.getByRole('button', { name: T('viewWork') }).locator('visible=true').first()).toBeVisible();
  });
});

test('US-2.3 每部已配作品頁：封面、標題、品牌、發售日、本機路徑、角色/製作人員/關聯', async ({ page }) => {
  test.setTimeout(15 * 60_000);
  await openApp(page, core);
  const { resources } = await catalog(core);
  const problems: string[] = [];
  const quick = { timeout: 20_000 };
  for (const w of works.filter(w => w.vndb_id)) {
    await searchCollection(page, '');
    await openWork(page, w);
    const facts = page.locator('.work-facts');
    const check = async (what: string, fn: () => Promise<unknown>) => { try { await fn(); } catch (e) { problems.push(`${w.vndb_id} ${w.title}: ${what} — ${String(e).split('\n')[0].slice(0, 160)}`); } };
    await check('title', () => expect(page.locator('h1:visible').first()).toHaveText(w.title, quick));
    await check('brand', () => expect(facts.locator('.work-companies')).not.toHaveText(T('unknown'), quick));
    await check('released', () => expect(facts).toContainText(/\d{4}-\d{2}/, quick));
    await check('cover', () => expect.poll(() => page.locator('.work-hero img').evaluateAll(imgs => imgs.some(i => (i as HTMLImageElement).naturalWidth > 0)), quick).toBe(true));
    for (const r of resources.filter(r => r.work_id === w.id && r.files > 0)) await check(`path ${r.path}`, () => expect(page.getByText(r.path.split('/').pop()!, { exact: false }).locator('visible=true').first()).toBeVisible(quick));
    for (const section of ['characters', 'staff', 'related'] as const) await check(section, () => expect(page.getByRole('heading', { name: T(section), exact: true }).locator('visible=true').first()).toBeVisible(quick));
    await check('no error alert', () => expect(page.locator('main [role=alert]:visible')).toHaveCount(0));
  }
  console.log(problems.join('\n'));
  expect(problems).toEqual([]);
});

test('US-2.5 背景收藏更新保留捲動位置與卡片；改篩選才回頂端', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await openApp(page, core);
  await nav(page, 'library');
  await expect(results(page).nth(12)).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  await results(page).nth(12).evaluate(element => window.scrollTo(0, window.scrollY + element.getBoundingClientRect().top - 150));
  // Let the scroll event and sticky header settle before taking the reference position.
  await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  const firstVisible = () => page.locator('.book-grid .book').evaluateAll(cards => {
    const top = document.querySelector('.collection-browser-tools')!.getBoundingClientRect().bottom;
    return cards.find(card => { const rect = card.getBoundingClientRect(); return rect.bottom > top && rect.top < innerHeight; })?.getAttribute('data-work-id');
  });
  const id = await firstVisible();
  expect(id).toBeTruthy();
  const visible = page.locator(`.book[data-work-id="${id}"]`);
  // This marker survives only if React retains the actual card node across the refresh.
  await visible.evaluate(element => element.setAttribute('data-scroll-probe', 'retained'));
  const y = await page.evaluate(() => window.scrollY);
  expect(y).toBeGreaterThan(500);
  const before = await core.api<{ revision: number; favorite: boolean }>(`/works/${id}`);
  try {
    await core.api(`/works/${id}`, { revision: before.revision, favorite: !before.favorite });
    // Polling /changes runs every 10 seconds; the rendered heart proves the refresh completed.
    await expect(visible.locator('.cover-heart')).toHaveCount(before.favorite ? 0 : 1, { timeout: 30_000 });
    expect(Math.abs(await page.evaluate(() => window.scrollY) - y)).toBeLessThanOrEqual(3);
    expect(await firstVisible()).toBe(id);
    await expect(visible).toHaveAttribute('data-scroll-probe', 'retained');
    await expect(visible).toBeInViewport();

    // Dispatch without Playwright scrolling the filter into view, so the app must reset scroll itself.
    await page.locator('.collection-tags').getByRole('button', { name: T('favorite'), exact: true }).evaluate(element => (element as HTMLButtonElement).click());
    await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
    await expect(page.locator('.collection-tags').getByRole('button', { name: T('favorite'), exact: true })).toHaveAttribute('aria-pressed', 'true');
    await expect.poll(async () => {
      const ids = await results(page).evaluateAll(cards => cards.map(card => card.getAttribute('data-work-id')));
      const favorites = (await core.api<Array<{ id: string; favorite: boolean }>>('/works')).filter(work => work.favorite).map(work => work.id);
      return ids.length === Math.min(60, favorites.length) && ids.every(id => favorites.includes(id!));
    }).toBe(true);
    expect(await page.evaluate(() => window.scrollY)).toBe(0);
  } finally {
    const current = await core.api<{ revision: number }>(`/works/${id}`);
    await core.api(`/works/${id}`, { revision: current.revision, favorite: before.favorite });
  }
});
