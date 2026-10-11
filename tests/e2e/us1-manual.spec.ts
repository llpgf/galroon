// US-1.5 配不到的資源手動綁定；US-1.6 把資源改掛到既有作品。
import { test, expect } from '@playwright/test';
import { openApp, nav, T, openMatcher, card as cardOf } from './support/app';
import { startCore, SNAPSHOTS, type Core } from './support/core';
import { catalog } from './support/golden';

let core: Core;
test.beforeEach(async () => { core = await startCore({ name: 'us1-manual', from: `${SNAPSHOTS}/intake` }); });
test.afterEach(async () => { await core?.stop(); });

// After automatic matching only the two 善悪 bonus archives remain (the title is too short to match safely).
const MAMA_CD = '善悪 フルHDリマスター版 Webアンケート特典';
const MAMA_MDF = '原法子との邂逅';
const bound = async (path: string) => { const { resources, byId } = await catalog(core); const r = resources.find(x => x.path.includes(path))!; return r.work_id ? byId.get(r.work_id)! : null; };

test('US-1.5 看原因 → 搜 VNDB → 選候選 → 確認，重新配對/掃描不覆蓋', async ({ page }) => {
  await openApp(page, core);

  await test.step('待處理列顯示未配對原因', async () => {
    await nav(page, 'organize');
    await page.getByRole('button', { name: T('unmatched'), exact: true }).click();
    const rows = page.locator('.organize tbody tr');
    await expect(rows.first()).toBeVisible();
    const count = await rows.count();
    for (let i = 0; i < count; i++) await expect(rows.nth(i).locator('.match-review-reason'), `reason on row ${i}`).toContainText(T('matchWhyUnmatched'));
  });

  const worksBefore = (await core.api('/works')).length;
  await test.step('第一份善悪特典：搜 VNDB 選 v20672', async () => {
    const drawer = await openMatcher(page, MAMA_CD);
    await drawer.getByLabel(T('query')).fill('善悪');
    await drawer.getByRole('button', { name: T('searchVndb') }).click();
    const candidate = drawer.locator('button.candidate').filter({ hasText: 'v20672' });
    await expect(candidate).toBeVisible({ timeout: 30_000 });
    await candidate.click();
    await expect(page.locator('.drawer')).toHaveCount(0);
    expect((await bound(MAMA_CD))?.vndb_id).toBe('v20672');
  });

  await test.step('第二份選同一候選 → 併入同一張卡', async () => {
    const drawer = await openMatcher(page, MAMA_MDF);
    await drawer.getByLabel(T('query')).fill('善悪');
    await drawer.getByRole('button', { name: T('searchVndb') }).click();
    await drawer.locator('button.candidate').filter({ hasText: 'v20672' }).click();
    await expect(page.locator('.drawer')).toHaveCount(0);
    const a = await bound(MAMA_CD), b = await bound(MAMA_MDF);
    expect(b?.id).toBe(a?.id);
    expect((await core.api('/works')).length).toBe(worksBefore + 1);
  });

  await test.step('Collection 立刻出現帶封面的卡', async () => {
    await nav(page, 'library');
    const card = cardOf(page, (await bound(MAMA_CD))!);
    await expect(card).toHaveCount(1);
    await expect.poll(() => card.locator('img').evaluate(i => (i as HTMLImageElement).naturalWidth), { timeout: 60_000 }).toBeGreaterThan(0);
  });

  await test.step('重新配對與重新掃描都不覆蓋手動綁定', async () => {
    const id = (await bound(MAMA_CD))!.id;
    const idle = async () => {
      await expect.poll(async () => (await core.api('/jobs')).filter((j: any) => ['queued', 'running', 'pausing'].includes(j.state)).length, { timeout: 300_000, intervals: [1000] }).toBe(0);
    };
    await core.api('/matching/refresh', {});
    await idle();
    const roots = await core.api('/roots');
    await core.api('/scans', { root_id: roots[0].id, scope: '', exclude: [] });
    await idle();
    expect((await bound(MAMA_CD))?.id).toBe(id);
    expect((await bound(MAMA_MDF))?.id).toBe(id);
  });
});

test('US-1.5 異常：確認前資源已被其他操作改動 → 顯示衝突，不靜默覆蓋', async ({ page }) => {
  await openApp(page, core);
  const drawer = await openMatcher(page, MAMA_CD);
  await drawer.getByLabel(T('query')).fill('善悪');
  await drawer.getByRole('button', { name: T('searchVndb') }).click();
  const candidate = drawer.locator('button.candidate').filter({ hasText: 'v20672' });
  await expect(candidate).toBeVisible({ timeout: 30_000 });
  // Meanwhile another client binds the same resource to a different work.
  const { resources, works } = await catalog(core);
  const r = resources.find(x => x.path.includes(MAMA_CD))!;
  const other = works.find(w => w.vndb_id === 'v62721')!;
  await core.api(`/resources/${r.id}/bind`, { work_id: other.id, release: r.release, revision: (r as any).revision });
  await candidate.click();
  await expect(page.locator('.drawer [role=alert], .drawer .error-text')).toBeVisible();
  expect((await bound(MAMA_CD))?.id, 'the other client\'s binding survives').toBe(other.id);
});

test('US-1.6 把資源掛到既有作品，並改掛已配資源', async ({ page }) => {
  await openApp(page, core);
  const { works } = await catalog(core);
  const target = works.find(w => w.vndb_id === 'v62721')!; // マガルミナ
  const worksBefore = works.length;

  await test.step('未配資源 → 現有作品 → 選定', async () => {
    const drawer = await openMatcher(page, 'WaffleTokuten');
    await drawer.getByLabel(T('query')).fill('マガルミナ');
    const choice = drawer.locator('.match-work-choices button.candidate').filter({ hasText: 'マガルミナ' });
    await expect(choice).toBeVisible();
    await choice.click();
    await expect(page.locator('.drawer')).toHaveCount(0);
    expect((await bound('WaffleTokuten'))?.id).toBe(target.id);
    expect((await core.api('/works')).length, 'no new card').toBe(worksBefore);
  });

  await test.step('已配資源改掛 → 原作品沒有資源時不留空殼卡', async () => {
    const drawer = await openMatcher(page, '1330947'); // ギャルズフィクション, its only resource
    await drawer.getByLabel(T('query')).fill('マガルミナ');
    await drawer.locator('.match-work-choices button.candidate').filter({ hasText: 'マガルミナ' }).click();
    await expect(page.locator('.drawer')).toHaveCount(0);
    expect((await bound('1330947'))?.id).toBe(target.id);
    await nav(page, 'library');
    await page.getByPlaceholder(T('search')).first().fill('ギャルズフィクション');
    const shell = cardOf(page, works.find(w => w.vndb_id === 'v55797')!);
    // Either the empty card is gone, or it says plainly that it has no local files.
    if (await shell.count()) await expect(shell).toContainText(T('availability_no_resources'));
  });

  await test.step('重新配對不覆蓋手動掛載', async () => {
    await core.api('/matching/refresh', {});
    await expect.poll(async () => (await core.api('/matching')).job?.state, { timeout: 300_000 }).not.toBe('running');
    expect((await bound('WaffleTokuten'))?.id).toBe(target.id);
    expect((await bound('1330947'))?.id).toBe(target.id);
  });
});
