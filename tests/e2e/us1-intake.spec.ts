// US-1.1 加入來源後自動出現封面卡 — also produces the "intake" snapshot other stories start from.
import { test, expect } from '@playwright/test';
import { openApp, nav, T } from './support/app';
import { startCore, snapshot, waitFor, TESTSET, type Core } from './support/core';
import { grade, summary, expectedCards, fingerprint } from './support/golden';

let core: Core;
test.afterAll(async () => { if (core) { await core.stop(); snapshot(core, 'intake'); } });

test('US-1.1 加入來源後不按任何按鈕就出現封面卡', async ({ page }) => {
  const before = fingerprint(TESTSET);
  core = await startCore({ name: 'us1-intake' });
  await openApp(page, core);

  await test.step('在 Sources 新增 NAS 測試集 並儲存', async () => {
    await nav(page, 'sources');
    await page.getByRole('button', { name: T('addSource') }).first().click();
    await page.getByLabel(T('sourceLabel')).fill('testset');
    await page.getByLabel(T('sourcePath')).fill(TESTSET);
    await page.getByRole('button', { name: T('save'), exact: true }).click();
    await expect(page.getByText(TESTSET)).toBeVisible();
  });

  await test.step('Activity 自動出現掃描與配對任務（不點掃描/配對）', async () => {
    await nav(page, 'tasks');
    await expect(page.getByRole('heading', { name: T('automaticScan') }).first()).toBeVisible({ timeout: 60_000 });
    await expect(page.getByRole('heading', { name: T('job_match') }).first()).toBeVisible({ timeout: 5 * 60_000 });
  });

  await test.step('10 分鐘內掃描與配對都完成', async () => {
    await waitFor('matching to finish', async () => {
      const m = await core.api('/matching');
      return m.job?.state === 'completed' && m.job.processed === m.job.total;
    }, 10 * 60_000, 3_000);
  });

  await test.step('Updates 顯示結果，數字是資源數', async () => {
    const status = await core.api('/matching');
    const finished = status.updates.find((u: any) => u.kind !== 'resource.discovered');
    expect(finished, 'matching-finished update').toBeTruthy();
    await page.getByRole('button', { name: new RegExp('^' + T('intakeUpdates')) }).click();
    await expect(page.getByText(T('intakeFinished', { matched: finished.data.matched, review: finished.data.review }))).toBeVisible();
  });

  const rows = await grade(core);
  const result = summary(rows);
  console.log(`\nGolden: ${result.ok} ok / ${result.unmatched} unmatched / ${result.falseMatches} FALSE\n${result.table}`);
  await test.info().attach('golden.txt', { body: result.table, contentType: 'text/plain' });

  await test.step('G1 絕不錯配', async () => { expect(rows.filter(r => r.verdict === 'FALSE MATCH')).toEqual([]); });

  await test.step('Collection 卡片數與 golden 一致，封面圖實際載入', async () => {
    await nav(page, 'library');
    const total = await core.api('/works');
    expect.soft(total.length, 'cards in collection').toBe(expectedCards());
    const cards = page.locator('.book-grid .book');
    await expect(cards).toHaveCount(total.length);
    for (let i = 0; i < total.length; i++) await cards.nth(i).scrollIntoViewIfNeeded();
    const withCover = total.filter((w: any) => w.cover).length;
    expect.soft(withCover, 'works with a cover URL').toBe(total.length);
    // An <img> only receives its source once Core has fetched the artwork, so poll until every one has real pixels.
    await expect.poll(() => page.locator('.book-grid .book img').evaluateAll(imgs => imgs.filter(i => (i as HTMLImageElement).naturalWidth > 0).length), { timeout: 120_000, message: 'covers loaded' }).toBe(withCover);
  });

  await test.step('每個 golden 資料夾都配上', async () => { expect.soft(rows.filter(r => r.verdict !== 'ok').map(r => r.folder + (r.member ? ' / ' + r.member : ''))).toEqual([]); });

  await test.step('G3 來源檔案未被改動', async () => { expect(fingerprint(TESTSET)).toEqual(before); });
});
