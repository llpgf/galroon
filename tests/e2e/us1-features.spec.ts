// US-1.2 同一作品多份資源合成一張卡；US-1.3 多作品特典資料夾拆開配對；US-1.9 重製版建立獨立卡；US-2.4 手動修改與 VNDB 衝突時讓我選。
import { test, expect, type Page } from '@playwright/test';
import { openApp, nav, T, card, searchCollection, openWork } from './support/app';
import { startCore, SNAPSHOTS, type Core } from './support/core';
import { catalog, GOLDEN } from './support/golden';

let core: Core;
test.beforeEach(async () => { core = await startCore({ name: 'us1-features', from: `${SNAPSHOTS}/intake` }); });
test.afterEach(async () => { await core?.stop(); });

const workBy = async (match: (w: any) => boolean) => (await core.api('/works')).find(match);
const moreMenu = async (page: Page) => { await page.locator('.detail-actions summary').click(); };

test('US-1.2 生命のスペア 兩份資源：一張卡、一個版本、兩個資源', async ({ page }) => {
  const works = (await core.api('/works')).filter((w: any) => w.vndb_id === 'v19513');
  expect(works, 'one card for v19513').toHaveLength(1);
  const { resources } = await catalog(core);
  const spare = resources.filter(r => ['生命のスペア', '生命のスペア.rar'].includes(r.path));
  expect(spare).toHaveLength(2);
  for (const r of spare) expect(r.work_id, r.path).toBe(works[0].id);
  const releases = new Set(spare.map(r => r.bindings[0]?.release_id ?? null));
  expect(releases.size, 'both resources share one edition').toBe(1);

  await openApp(page, core);
  await searchCollection(page, '生命のスペア');
  await expect(page.locator('.book-grid .book')).toHaveCount(1);
  await openWork(page, works[0]);
  for (const r of spare) await expect(page.getByText(r.path).first()).toBeVisible();
});

test('US-1.3 WaffleTokuten 拆成 7 個資源，自動配到 3 部作品（善悪留待手動），卡片標「只有特典」且可篩掉', async ({ page }) => {
  const split = GOLDEN.folders['WaffleTokuten'].split!;
  const { resources, byId } = await catalog(core);
  const owned = new Set<string>();
  for (const [member, want] of Object.entries(split)) {
    const r = resources.find(x => x.path.normalize('NFC') === `WaffleTokuten/${member}`.normalize('NFC'));
    expect(r, member).toBeTruthy();
    const w = r!.work_id ? byId.get(r!.work_id) : undefined;
    if (want.manual) { expect(w, `${member} stays for manual review`).toBeUndefined(); continue; }
    expect(w?.vndb_id, member).toBe(want.work);
    expect(r!.bindings[0]?.role, `${member} is bound as a bonus`).toBe('extra');
    owned.add(w!.id);
  }
  expect(owned.size).toBe(3);
  expect(resources.filter(r => r.path === 'WaffleTokuten' && (r as any).files > 0), 'no whole-folder resource left').toHaveLength(0);

  await openApp(page, core);
  await nav(page, 'library');
  const total = (await core.api('/works')).length;
  for (const id of owned) {
    const w = byId.get(id)!;
    await expect(card(page, w).getByText(T('bonusOnly'))).toBeVisible();
  }
  const bonusOnly = new Set(Object.values(GOLDEN.folders).flatMap(want => want.split ? Object.values(want.split) : [want])
    .filter(want => want.bonus_only && !want.manual).flatMap(want => want.works || (want.work ? [want.work] : [])));
  await page.getByRole('button', { name: T('hideBonusOnly') }).click();
  await expect(page.locator('.book-grid .book')).toHaveCount(total - bonusOnly.size);
  await expect(page.getByText(T('filteredWorks', { total, count: total - bonusOnly.size })).first()).toBeVisible();
  for (const id of owned) await expect(card(page, byId.get(id)!)).toHaveCount(0);
});

test('US-1.9 重製版：獨立卡用重製版發行資料，重抓不改回原版，與原版互相關聯', async ({ page }) => {
  for (const [folder, want] of Object.entries(GOLDEN.folders).filter(([, w]) => w.remake_release)) {
    await test.step(folder, async () => {
      const w = await workBy(x => x.vndb_release === want.remake_release);
      expect(w, `card for ${want.remake_release}`).toBeTruthy();
      expect(w.vndb_id).toBe(want.work);
      expect(w.original_title).toBe(want.card_title);
      expect(w.released).toBe(want.card_released);
      expect(w.developer).toMatch(/FG Remake/i);
      // Refresh from VNDB keeps the remake's own card data.
      const refreshed = await core.api(`/works/${w.id}/refresh`, { revision: w.revision });
      expect(refreshed.conflicts ?? []).toEqual([]);
      const after = await workBy(x => x.id === w.id);
      expect([after.original_title, after.released]).toEqual([want.card_title, want.card_released]);
    });
  }

  await test.step('作品頁仍有角色與製作人員（沿用原條目）', async () => {
    const remake = await workBy(x => x.vndb_release === 'r140695');
    await openApp(page, core);
    await nav(page, 'library');
    await openWork(page, remake);
    await expect(page.getByRole('heading', { name: T('characters'), exact: true }).first()).toBeVisible({ timeout: 60_000 });
    await expect(page.locator('h1:visible').first()).toHaveText(remake.title);
  });

  await test.step('另有原版卡時兩張卡互相顯示', async () => {
    const original = await core.api('/works', { title: 'Kakyuusei', original_title: '下級生', vndb_id: 'v2341' });
    const remake = await workBy(x => x.vndb_release === 'r140695');
    expect(original.id).not.toBe(remake.id);
    await nav(page, 'library');
    await openWork(page, remake);
    const section = page.locator('.same-work');
    await expect(section).toContainText('下級生');
    await section.getByRole('button').first().click();
    await expect(page.locator('h1:visible').first()).toHaveText('Kakyuusei');
    await expect(page.locator('.same-work')).toContainText('下級生リメイク');
  });
});

test('US-2.4 手動改過標題後從 VNDB 更新：顯示差異讓我選', async ({ page }) => {
  const w = await workBy(x => x.vndb_id === 'v61834'); // Relirium
  const providerTitle = w.title;
  const mine = await core.api(`/works/${w.id}/metadata`, { revision: w.revision, patch: { title: '我的Relirium' } });
  expect(mine.revision).toBeGreaterThan(w.revision);

  await openApp(page, core);
  await searchCollection(page, 'Relirium');
  await openWork(page, { title: '我的Relirium' });

  const refresh = async () => { await moreMenu(page); await page.getByRole('button', { name: T('refreshMetadata') }).click(); };
  await test.step('保留我的', async () => {
    await refresh();
    const region = page.getByRole('region', { name: T('refreshConflicts') });
    await expect(region).toBeVisible({ timeout: 30_000 });
    await expect(region).toContainText('我的Relirium');
    await expect(region).toContainText(providerTitle);
    await expect(region.locator('.plan-item'), 'only the edited field is asked').toHaveCount(1);
    await region.getByRole('button', { name: T('keepMine') }).click();
    await expect(region).toHaveCount(0);
    await expect(page.locator('h1:visible').first()).toHaveText('我的Relirium');
  });
  await test.step('改用 VNDB', async () => {
    await refresh();
    const region = page.getByRole('region', { name: T('refreshConflicts') });
    await region.getByRole('button', { name: T('useProvider') }).click();
    await expect(region).toHaveCount(0);
    await expect(page.locator('h1:visible').first()).toHaveText(providerTitle);
    expect((await workBy(x => x.id === w.id)).title).toBe(providerTitle);
  });
  await test.step('沒有修改過的欄位不提示', async () => {
    await refresh();
    await page.waitForTimeout(3_000);
    await expect(page.getByRole('region', { name: T('refreshConflicts') })).toHaveCount(0);
  });
});
