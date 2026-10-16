// US-1.7 VNDB 連不上或被限速；US-1.8 中途關閉／Core 重啟。
import { test, expect, type Page } from '@playwright/test';
import { openApp, nav, T } from './support/app';
import { startCore, waitFor, TESTSET, type Core } from './support/core';
import { grade, summary, expectedRows } from './support/golden';
import { FaultyVndb } from './support/vndb';

let core: Core | undefined, vndb: FaultyVndb;
test.beforeEach(async () => { vndb = await new FaultyVndb().start(); });
test.afterEach(async () => { await core?.stop(); core = undefined; await vndb.stop(); });

const env = () => ({ GALROON_VNDB_API: vndb.url });
const matchJob = async (c: Core) => (await c.api('/matching')).job;
async function addSource(c: Core) { await c.api('/matching/start', {}); await c.api('/roots', { path: TESTSET, label: 'testset' }); }
async function bindings(c: Core) { return (await c.api('/resources')).filter((r: any) => r.work_id).length; }
/** The match task card in Activity. */
const matchCard = (page: Page) => page.locator('article.job').filter({ has: page.getByRole('heading', { name: T('job_match') }) }).first();

for (const [mode, label] of [['timeout', '逾時'], ['refused', '斷線'], ['429', '限速 429']] as const) {
  test(`US-1.7 VNDB ${label}：暫停並說明原因，恢復後續配且不錯配`, async ({ page }) => {
    vndb.mode = mode;
    core = await startCore({ name: `us1-vndb-${mode}`, env: env() });
    await addSource(core);
    await waitFor('match task paused', async () => (await matchJob(core!))?.state === 'paused', 180_000);
    const requestsAtPause = vndb.requests;

    await test.step('故障期間不建立任何綁定', async () => { expect(await bindings(core!)).toBe(0); });

    await test.step('Activity 不展開就看得到原因與「繼續」', async () => {
      await openApp(page, core!);
      await nav(page, 'tasks');
      const card = matchCard(page);
      await expect(card.getByText(T('paused'))).toBeVisible();
      await expect(page.locator('.issues, section').filter({ hasText: 'No VNDB candidate found' }), 'outage must not be reported as "no candidate"').toHaveCount(0);
      await expect(card.getByText(/VNDB/).first(), 'reason visible without opening details').toBeVisible();
      await expect(card.getByRole('button', { name: T('resume') })).toBeVisible();
    });

    await test.step('不狂打重試', async () => {
      await page.waitForTimeout(20_000);
      expect(vndb.requests - requestsAtPause, 'requests while paused').toBeLessThanOrEqual(1);
    });

    await test.step('VNDB 恢復 → 按繼續 → 完成且錯配 0', async () => {
      vndb.mode = 'proxy';
      await matchCard(page).getByRole('button', { name: T('resume') }).click();
      await waitFor('matching complete', async () => (await matchJob(core!))?.state === 'completed', 600_000, 3_000);
      const result = summary(await grade(core!));
      console.log(result.table);
      expect(result.falseMatches).toBe(0);
      // Manual rows may count as ok while unbound; require at least every non-manual row.
      expect(result.ok).toBeGreaterThanOrEqual(expectedRows({ includeManual: false }));
    });
  });
}

test('US-1.7 VNDB 部分回應：不完整的項目留待處理並標原因，不錯配', async () => {
  vndb.mode = 'flaky';
  core = await startCore({ name: 'us1-vndb-flaky', env: env() });
  await addSource(core);
  // Each failure pauses the task; keep resuming like a user would until it finishes.
  await waitFor('matching complete under flaky VNDB', async () => {
    const job = await matchJob(core!);
    if (job?.state === 'paused') await core!.api(`/jobs/${job.id}/resume`, {});
    return job?.state === 'completed';
  }, 900_000, 3_000);
  const result = summary(await grade(core));
  console.log(result.table);
  expect(result.falseMatches).toBe(0);
  const unmatched = (await core.api('/resources')).filter((r: any) => !r.work_id);
  const page = await core.api('/resources/page?status=unmatched&limit=100').catch(() => null);
  for (const r of page?.items || unmatched) expect(r.auto_match_reason, `reason for ${r.path}`).toBeTruthy();
});

test('US-1.8 配對中途 Core 被關閉 → 重開為「已中斷」→ 繼續 → 結果與一次跑完相同', async ({ page }) => {
  vndb.delayMs = 1500; // slow enough to stop half-way
  core = await startCore({ name: 'us1-restart-match', env: env() });
  await addSource(core);
  await waitFor('match half-way', async () => { const j = await matchJob(core!); return j?.state === 'running' && j.processed >= 8; }, 300_000, 1_000);
  await core.kill();
  vndb.delayMs = 0;
  core = await startCore({ dir: core.dir, env: env(), fresh: false });

  await test.step('重開後任務為已中斷，不自動重跑', async () => {
    await page.waitForTimeout(5_000);
    expect((await matchJob(core!))?.state).toBe('interrupted');
  });

  await test.step('在 Activity 按繼續', async () => {
    await openApp(page, core!);
    await nav(page, 'tasks');
    const card = matchCard(page);
    await expect(card.getByText(T('interrupted'))).toBeVisible();
    await card.getByRole('button', { name: T('resume') }).click();
    // Real VNDB may rate-limit (429) after many runs; the task then pauses with a reason, and a person resumes later.
    let lastResume = Date.now();
    await waitFor('matching complete', async () => {
      const job = await matchJob(core!);
      if (job?.state === 'paused' && Date.now() - lastResume > 30_000) { await core!.api(`/jobs/${job.id}/resume`, {}); lastResume = Date.now(); }
      return job?.state === 'completed';
    }, 900_000, 3_000);
  });

  await test.step('作品數與綁定數同一次跑完，無重複卡', async () => {
    const works = await core!.api('/works');
    const ids = works.map((w: any) => w.vndb_id).filter(Boolean);
    expect(new Set(ids).size, 'duplicate VNDB cards').toBe(ids.length);
    const result = summary(await grade(core!));
    expect(result.falseMatches).toBe(0);
    expect(result.ok).toBeGreaterThanOrEqual(expectedRows({ includeManual: false }));
  });
});

test('US-1.8 掃描中途 Core 被關閉 → 繼續 → 資源數與一次跑完相同', async ({ page }) => {
  core = await startCore({ name: 'us1-restart-scan', env: env() });
  vndb.mode = 'refused'; // keep matching out of the way; this story is about the scan
  await addSource(core);
  await waitFor('scan running', async () => (await core!.api('/jobs')).some((j: any) => j.kind === 'scan' && j.state === 'running' && j.processed > 0), 120_000, 200);
  await core.kill();
  core = await startCore({ dir: core.dir, env: env(), fresh: false });
  await page.waitForTimeout(5_000);
  const scans = (await core.api('/jobs')).filter((j: any) => j.kind === 'scan');
  console.log(scans.map((j: any) => `${j.state} ${j.processed}`));
  const interrupted = scans.find((j: any) => j.state === 'interrupted');
  if (interrupted) {
    await openApp(page, core);
    await nav(page, 'tasks');
    await page.locator('article.job').filter({ hasText: T('interrupted') }).first().getByRole('button', { name: T('resume') }).click();
  }
  await waitFor('scans idle', async () => (await core!.api('/jobs')).every((j: any) => j.kind === 'artwork' || !['queued', 'running', 'pausing'].includes(j.state)), 300_000);
  // Same resources as an uninterrupted scan: every golden folder, with split folders counted per member.
  const expected = expectedRows();
  expect((await core.api('/resources')).filter((r: any) => r.files > 0).length).toBe(expected);
});
