// US-5.1 AI suggests; a person confirms. US-5.2 one physical resource on several work pages.
import { test, expect, type Page } from '@playwright/test';
import { openApp, nav, T, resourceRow, card, openWork } from './support/app';
import { startCore, SNAPSHOTS, type Core } from './support/core';
import { grade } from './support/golden';
import type { Resource, MatchSuggestion, Work } from '../../src/api';

let core: Core;
const MAMA_CD = '善悪 フルHDリマスター版 Webアンケート特典';
const resources = () => core.api<Resource[]>('/resources');
const resource = async (text: string) => { const r = (await resources()).find(r => r.path.includes(text)); expect(r, text).toBeTruthy(); return r!; };
const current = async (id: string) => (await resources()).find(r => r.id === id)!;
const idle = async () => expect.poll(async () => (await core.api('/jobs')).filter((j: any) => ['queued', 'running', 'pausing', 'cancelling'].includes(j.state)).length, { timeout: 300_000, intervals: [1000] }).toBe(0);
const suggest = (r: Resource, works = ['v20672'], query = '善悪') => core.api<MatchSuggestion>(`/resources/${r.id}/suggestions`, { revision: r.revision, works, query, rationale: 'Golden identifies this resource with these works; please review the proposed match.' });
const review = async (page: Page) => { await openApp(page, core); await nav(page, 'organize'); await page.getByRole('button', { name: T('unmatched'), exact: true }).click(); };
const references = (r: Resource, bindings: Resource['bindings'], primary: string | null) => core.api(`/resources/${r.id}/references`, { revision: r.revision, primary_work_id: primary, bindings });

test.beforeEach(async () => { core = await startCore({ name: 'us5-suggestions', from: `${SNAPSHOTS}/intake` }); await idle(); });
test.afterEach(async () => { await core?.stop(); });

test('US-5.1 AI 建議 → 確認 → 出卡，重新掃描保留綁定', async ({ page }) => {
  const r = await resource(MAMA_CD);
  expect(r.work_id).toBeNull();
  const proposal = await suggest(r);
  expect((await current(r.id)).bindings).toHaveLength(0); // Creating a suggestion never binds.
  await review(page);
  const row = resourceRow(page, MAMA_CD);
  await expect(row.locator('.suggestion-badge')).toContainText(T('suggestionBadge'));
  await expect(row.locator('.suggestion-works')).toContainText('v20672');
  await expect(row.locator('.suggestion-rationale')).not.toBeVisible();
  await row.getByText(T('suggestionRationale'), { exact: true }).click();
  await expect(row.locator('.suggestion-rationale')).toHaveText(proposal.rationale);
  await page.screenshot({ path: test.info().outputPath('ai-suggestion.png'), fullPage: true });
  const confirmed = page.waitForResponse(res => res.url().endsWith(`/suggestions/${proposal.id}/confirm`) && res.status() === 200);
  await row.getByRole('button', { name: T('suggestionConfirm'), exact: true }).click();
  await confirmed;
  await expect(row).toHaveCount(0);
  const bound = await current(r.id);
  const w = await core.api<Work>(`/works/${bound.work_id}`);
  expect(w.vndb_id).toBe('v20672');
  await nav(page, 'library');
  await expect(card(page, w)).toHaveCount(1);
  await core.api('/scans', { root_id: r.root_id, scope: '', exclude: [] });
  await idle();
  expect((await current(r.id)).bindings).toEqual(bound.bindings);
  expect((await grade(core)).filter(r => r.verdict !== 'ok')).toEqual([]);
});

test('批次只確認已驗證建議，未驗證建議保留', async ({ page }) => {
  const verified = await suggest(await resource(MAMA_CD));
  // This append is a release of v47547; release-only titles cannot verify suggestions.
  const append = await resource('1327332');
  await references(append, [], null);
  const unverified = await suggest(await current(append.id), ['v47547'], 'アンラベル・トリガー -Prelude to War-');
  expect(verified.verified).toBe(true);
  expect(unverified.verified).toBe(false);
  await review(page);
  await expect(resourceRow(page, '1327332').locator('.suggestion-badge')).toContainText(T('suggestionUnverified'));
  // Filtering out the verified resource must not silently limit "all" to visible rows.
  await page.getByRole('searchbox', { name: T('resourceSearch') }).fill('1327332');
  const request = page.waitForRequest(req => req.url().endsWith('/suggestions/batch-confirm'));
  await page.getByRole('button', { name: T('suggestionConfirmVerified') }).click();
  expect((await request).postDataJSON()).toEqual({ ids: [verified.id], revisions: [verified.resource_revision] });
  await expect(page.getByRole('status', { name: T('suggestionResults') })).toContainText(T('suggestionConfirmed'));
  await expect(page.getByRole('button', { name: T('suggestionConfirmVerified') })).toHaveCount(0);
  expect((await current(verified.resource_id)).work_id).toBeTruthy();
  expect((await current(unverified.resource_id)).work_id).toBeNull();
  expect((await core.api(`/resources/${unverified.resource_id}/suggestions`)).suggestion.id).toBe(unverified.id);
  await expect(resourceRow(page, '1327332').locator('.suggestion-badge')).toBeVisible();
  await references(await current(append.id), append.bindings, append.work_id);
  expect((await grade(core)).filter(r => r.verdict !== 'ok')).toEqual([]);
});

test('US-5.1 過期建議顯示衝突並重新載入，不覆蓋新版本', async ({ page }) => {
  const r = await resource(MAMA_CD), proposal = await suggest(r);
  await review(page);
  // Make the real revision change just as the reviewed confirmation is sent, avoiding a polling race.
  await page.route(`**/api/suggestions/${proposal.id}/confirm`, async route => {
    await references(await current(r.id), [], null);
    await route.fallback();
  });
  const conflict = page.waitForResponse(res => res.url().endsWith(`/suggestions/${proposal.id}/confirm`) && res.status() === 409);
  await resourceRow(page, MAMA_CD).getByRole('button', { name: T('suggestionConfirm'), exact: true }).click();
  expect((await (await conflict).json()).code).toBe('suggestion_changed');
  await expect(page.getByRole('alert')).toContainText(T('suggestionConflict'));
  await expect(resourceRow(page, MAMA_CD).locator('.match-suggestion')).toHaveCount(0);
  expect((await current(r.id)).work_id).toBeNull();
  expect((await current(r.id)).revision).toBeGreaterThan(r.revision);
  await page.screenshot({ path: test.info().outputPath('suggestion-conflict.png'), fullPage: true });
});

test('US-5.1 略過建議，不建立綁定', async ({ page }) => {
  const r = await resource(MAMA_CD), proposal = await suggest(r);
  await review(page);
  const dismissed = page.waitForResponse(res => res.url().endsWith(`/suggestions/${proposal.id}/dismiss`) && res.status() === 200);
  await resourceRow(page, MAMA_CD).getByRole('button', { name: T('suggestionDismiss'), exact: true }).click();
  await dismissed;
  await expect(resourceRow(page, MAMA_CD).locator('.match-suggestion')).toHaveCount(0);
  expect((await current(r.id)).bindings).toHaveLength(0);
});

async function regroup(sourceId: string, fileIds: string[], targetId: string | null, title: string) {
  const source = await core.api(`/resources/${sourceId}/members`);
  const target = targetId ? await core.api(`/resources/${targetId}/members`) : null;
  const selection = { revision: source.revision, file_ids: fileIds, target_id: targetId, target_revision: target?.revision ?? null, title };
  const preview = await core.api(`/resources/${sourceId}/regroup/preview`, selection);
  return core.api<{ id: string }>(`/resources/${sourceId}/regroup`, { ...selection, digest: preview.digest });
}

test('US-5.2 確認兩作品建議，每張作品頁都有合集，檔案只算一次', async ({ page }) => {
  // A real catalog-only collection of two golden resources, each referencing its correct work.
  // No arbitrary second work, fake VNDB response, source write, or golden edit.
  const a = await resource('1261651'), b = await resource('1326550');
  const bFiles: string[] = (await core.api(`/resources/${b.id}/members`)).files.map((f: any) => f.id);
  const before = await resources(), totalFiles = before.reduce((n, r) => n + r.files, 0);
  await references(a, [], null);
  await regroup(b.id, bFiles, a.id, a.title);
  const bundle = await current(a.id);
  expect(bundle.files).toBe(a.files + b.files);
  await suggest(bundle, ['v47547', 'v55281'], 'アンラベル・トリガー');
  await review(page);
  const row = resourceRow(page, a.title);
  await expect(row.locator('.suggestion-works li')).toHaveCount(2);
  await row.getByRole('button', { name: T('suggestionConfirm'), exact: true }).click();
  await expect(row).toHaveCount(0);
  const bound = await current(a.id);
  expect(bound.bindings).toHaveLength(2);
  expect((await resources()).reduce((n, r) => n + r.files, 0)).toBe(totalFiles);
  for (const [index, binding] of bound.bindings.entries()) {
    const work = await core.api<Work>(`/works/${binding.work_id}`);
    expect(['v47547', 'v55281']).toContain(work.vndb_id);
    expect(binding.role).toBe('main');
    const assets = await core.api(`/works/${work.id}/assets`);
    const listed = assets.resources.filter((r: Resource) => r.id === a.id);
    expect(listed).toHaveLength(1);
    expect(listed[0].files).toBe(bundle.files);
    expect(listed[0].bytes).toBe(bundle.bytes);
    await nav(page, 'library');
    await openWork(page, work);
    const resource = page.locator('.edition-resource').filter({ hasText: a.title });
    await expect(resource).toHaveCount(1);
    await expect(resource.locator('.collection-resource')).toHaveText(T('resourceCollection'));
    await page.screenshot({ path: test.info().outputPath(`collection-work-${index}.png`), fullPage: true });
  }
  // Restore the golden resource boundaries before applying its single-work grading rules.
  await regroup(a.id, bFiles, b.id, b.title);
  const aFiles = (await core.api(`/resources/${a.id}/members`)).files.map((f: any) => f.id);
  const restored = await regroup(a.id, aFiles, null, a.title);
  await references(await current(restored.id), a.bindings, a.work_id);
  expect((await grade(core)).filter(r => r.verdict !== 'ok')).toEqual([]);
});
