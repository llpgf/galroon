// US-3.1 從搜尋結果整批建立主題合集；US-3.2 清單條目隨作品變動而跟著走；US-3.3 自訂標籤批次貼上與篩選；
// US-3.4 用 VNDB 標籤篩選（含劇透控制）；US-3.5 標籤改名／合併／刪除。
import { test, expect, type Page, type Locator } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { openApp, nav, T, card, searchCollection, openWork } from './support/app';
import { startCore, SNAPSHOTS, RUNS, TESTSET, type Core } from './support/core';
import { catalog } from './support/golden';

let core: Core;
const CLOVER = 'CloverGAME';
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

// ---------- shared UI helpers ----------
const works = async () => (await catalog(core)).works;
const byVndb = async (id: string) => (await works()).find(w => w.vndb_id === id)!;
const cards = (page: Page) => page.locator('.book-grid .book');

async function idle() {
  await expect.poll(async () => (await core.api('/jobs')).filter((j: any) => j.kind !== 'artwork' && ['queued', 'running', 'pausing', 'cancelling'].includes(j.state)).length, { timeout: 120_000, intervals: [500] }).toBe(0);
}

/** Lists → create a manual list by name. */
async function createList(page: Page, name: string) {
  await nav(page, 'lists');
  const panel = page.locator('.lists-page');
  await panel.getByLabel(T('listName')).fill(name);
  await panel.getByRole('button', { name: T('listCreate') }).click();
  await expect(panel.getByRole('heading', { name, exact: true })).toBeVisible();
}

/** Lists → open a list from the index (re-entering the view so data is fresh). */
async function openList(page: Page, name: string) {
  await nav(page, 'home');
  await nav(page, 'lists');
  const panel = page.locator('.lists-page');
  await panel.locator('.list-index button').filter({ hasText: name }).click();
  await expect(panel.getByRole('heading', { name, exact: true })).toBeVisible();
  return panel;
}

const entry = (panel: Locator, title: string) => panel.locator('.list-members li').filter({ hasText: title });

/** Collection → tag focus dialog. */
async function tagDialog(page: Page) {
  await nav(page, 'library');
  await page.getByRole('button', { name: T('focusTags') }).click();
  const dialog = page.locator('dialog.focus-tag-dialog[open]');
  await expect(dialog).toBeVisible();
  return dialog;
}

/** Tag option button in the focus dialog: label followed by its count. */
const tagOption = (dialog: Locator, label: string) => dialog.locator('.focus-tag-options button').filter({ hasText: new RegExp(`^${escape(label)}(${escape(T('customTag'))})?\\d+$`) });

async function filterByTag(page: Page, label: string) {
  const dialog = await tagDialog(page);
  await dialog.getByLabel(T('findTags')).fill(label);
  await tagOption(dialog, label).click();
  await dialog.getByRole('button', { name: T('done'), exact: true }).click();
  await expect(dialog).toHaveCount(0);
}

/** Collection → tag dialog → manage custom tags, with `name` selected. */
async function manageTag(page: Page, name: string) {
  const dialog = await tagDialog(page);
  await dialog.getByRole('button', { name: T('manageTags') }).click();
  const manager = dialog.locator('.custom-tag-form');
  const select = manager.getByLabel(T('customTagName'));
  await expect(select.locator('option', { hasText: name })).toHaveCount(1);
  await select.selectOption({ label: name });
  return { dialog, manager };
}

async function createTagApi(name: string, ids: string[]) { return core.api('/custom-tags', { name, work_ids: ids }); }
async function tagsApi() { return core.api<any[]>('/custom-tags?include_deleted=true&summary=true'); }
async function members(tagId: string, ids: string[]) {
  const tag = (await tagsApi()).find(t => t.id === tagId);
  const q = new URLSearchParams({ ids: JSON.stringify(ids), revision: String(tag.revision), impact_revision: String(tag.impact_revision) });
  return (await core.api<{ members: string[] }>(`/custom-tags/${tagId}/membership?${q}`)).members.sort();
}

// =====================================================================
test.describe('intake 快照', () => {
  test.beforeEach(async () => { core = await startCore({ name: 'us3-lists-tags', from: `${SNAPSHOTS}/intake` }); });
  test.afterEach(async () => { await core?.stop(); });

  test('US-3.1 依品牌篩選 → 全部加入新清單「CloverGAME 合集」，重開後仍在，重複加入不產生重複', async ({ page }) => {
    const all = await works();
    const clover = all.filter(w => w.developer === CLOVER);
    // The NAS test set matches the story: 3 CloverGAME works (v48419, v59027, v60462).
    expect(clover.map(w => w.vndb_id).sort()).toEqual(['v48419', 'v59027', 'v60462']);
    const LIST = 'CloverGAME 合集';
    await openApp(page, core);

    await test.step('建立清單 → 品牌篩選 → 加入篩選結果 → 預覽 → 確認', async () => {
      await createList(page, LIST);
      await nav(page, 'library');
      await page.locator('button.focus-toggle').click();
      await page.locator('#collection-focus-panel').getByLabel(T('facet_studio')).selectOption({ label: CLOVER });
      await expect(cards(page)).toHaveCount(clover.length);
      await expect(page.getByText(T('filteredWorks', { total: all.length, count: clover.length })).first()).toBeVisible();
      await page.locator('.library-results .toolbar-menu summary').click();
      await page.getByRole('button', { name: T('listAddFiltered') }).click();
      const dialog = page.locator('dialog.add-to-list-dialog');
      await expect(dialog).toBeVisible();
      await expect(dialog).toContainText(T('listScopeCount', { count: clover.length }));
      await dialog.locator('summary').click();
      for (const w of clover) await expect(dialog.locator('ol li').filter({ hasText: w.title })).toHaveCount(1);
      await dialog.getByLabel(T('listDestination')).selectOption({ label: LIST });
      await dialog.getByRole('button', { name: T('listConfirmAdd') }).click();
      await expect(dialog.getByRole('status')).toHaveText(T('listAddedResult', { added: clover.length, skipped: 0 }));
      await dialog.getByRole('button', { name: T('done') }).click();
    });

    const checkList = async () => {
      const panel = await openList(page, LIST);
      await expect(panel).toContainText(T('listCount', { count: clover.length }));
      await expect(panel.locator('.list-members li')).toHaveCount(clover.length);
      for (const w of clover) await expect(entry(panel, w.title)).toHaveCount(1);
      await panel.getByRole('button', { name: T('listBack') }).click();
      const tile = panel.locator('.list-index button').filter({ hasText: LIST });
      await expect(tile).toContainText(T('listCount', { count: clover.length }));
      // The cover collage shows real artwork of the list's works.
      await expect.poll(() => tile.locator('.list-cover img').evaluateAll(imgs => imgs.filter(i => (i as HTMLImageElement).naturalWidth > 0).length), { timeout: 20_000 }).toBe(Math.min(4, clover.length));
      const covers = await tile.locator('.list-cover img').evaluateAll(imgs => imgs.map(i => (i as HTMLImageElement).getAttribute('src')));
      expect(new Set(covers).size, 'distinct covers').toBe(Math.min(4, clover.length));
    };

    await test.step('清單顯示全部、封面正確', checkList);

    await test.step('重開程式後仍在', async () => {
      core = await core.restart();
      await page.close();
      const again = await page.context().newPage();
      page = again;
      await openApp(page, core);
      await checkList();
    });

    await test.step('重複加入同一作品 → 不產生重複，告知已存在', async () => {
      const w = clover[0];
      await searchCollection(page, w.title);
      await openWork(page, w);
      await page.locator('.detail-actions').getByRole('button', { name: T('listAdd') }).click();
      const dialog = page.locator('dialog.add-to-list-dialog');
      await dialog.getByLabel(T('listDestination')).selectOption({ label: LIST });
      await dialog.getByRole('button', { name: T('listConfirmAdd') }).click();
      await expect(dialog.getByRole('status')).toHaveText(T('listAddedResult', { added: 0, skipped: 1 }));
      await dialog.getByRole('button', { name: T('done') }).click();
      const panel = await openList(page, LIST);
      await expect(panel.locator('.list-members li')).toHaveCount(clover.length);
      await expect(entry(panel, w.title)).toHaveCount(1);
    });
  });

  test('US-3.2 清單條目隨作品合併、來源移除與恢復而跟著走，筆記不遺失', async ({ page }) => {
    const LIST = '條目追蹤';
    const source = await byVndb('v48419'); // こあくまちゃんの誘惑っ！ (CloverGAME)
    const target = await byVndb('v59027'); // Yarinaoshi Crank-In (CloverGAME)
    const offline = await byVndb('v63827'); // Nemophilia — its only resource is folder 1366993
    const NOTE_A = '合併前寫的筆記';
    const NOTE_B = '來源移除前寫的筆記';
    await openApp(page, core);

    await test.step('建立清單並加入兩部作品、寫條目筆記', async () => {
      await createList(page, LIST);
      const panel = page.locator('.lists-page');
      for (const w of [source, offline]) {
        await panel.locator('.list-work-choice').getByLabel(T('findWorks')).fill(w.title);
        const choice = panel.locator('.list-work-choice select');
        await expect(choice.locator(`option[value="${w.id}"]`)).toHaveCount(1);
        await choice.selectOption(w.id);
        await panel.getByRole('button', { name: T('listAdd'), exact: true }).click();
        await expect(entry(panel, w.title)).toHaveCount(1);
      }
      for (const [w, note] of [[source, NOTE_A], [offline, NOTE_B]] as const) {
        await entry(panel, w.title).getByRole('button', { name: T('listEditEntry') }).click();
        await panel.getByLabel(T('listEntryNotes')).fill(note);
        await panel.getByRole('button', { name: T('listSave') }).click();
        await expect(entry(panel, w.title)).toContainText(note);
      }
    });

    await test.step('把作品合併到另一作品 → 條目指向合併後作品，筆記保留', async () => {
      await searchCollection(page, source.title);
      await openWork(page, source);
      await page.locator('.detail-actions summary').click();
      await page.getByRole('button', { name: T('groupingTitle') }).click();
      const drawer = page.locator('.drawer');
      await drawer.getByLabel(T('findWorks')).fill(target.title);
      const choice = drawer.locator('.list-work-choice select');
      await expect(choice.locator(`option[value="${target.id}"]`)).toHaveCount(1);
      await choice.selectOption(target.id);
      await drawer.getByRole('button', { name: T('reviewGrouping') }).click();
      await drawer.getByRole('button', { name: T('confirmGrouping') }).click();
      await expect(drawer).toHaveCount(0);

      const panel = await openList(page, LIST);
      await expect(panel.locator('.list-members li')).toHaveCount(2);
      const row = panel.locator('.list-members li').first();
      await expect(row).toContainText(NOTE_A);
      await row.locator('strong button').click();
      await expect(page.locator('h1:visible').first()).toHaveText(target.title);
      await page.getByRole('button', { name: T('back') }).first().click();
    });

    const testset = (await core.api('/roots'))[0];
    const remap = async (path: string) => {
      await idle();
      const preview = await core.api(`/roots/${testset.id}/remap/preview`, { path });
      await core.api(`/roots/${testset.id}/remap`, { path, digest: preview.digest });
      // Core rescans a remapped source automatically; give it a moment to queue, then wait.
      await new Promise(r => setTimeout(r, 1_500));
      await idle();
    };

    await test.step('移除來源（來源資料夾消失）→ 條目仍在，標示本機檔案不在，筆記保留', async () => {
      // There is no "remove source" action in the UI; the closest real mechanism is the source
      // folder disappearing: point the source at an empty folder and let Core rescan it.
      const empty = join(RUNS, `us3-empty-source-${Date.now()}`);
      mkdirSync(empty, { recursive: true });
      await remap(empty);
      await expect.poll(async () => (await core.api('/roots'))[0].missing, { timeout: 20_000 }).toBeGreaterThan(0);

      const panel = await openList(page, LIST);
      const row = entry(panel, offline.title);
      await expect(row).toHaveCount(1);
      await expect(row).toContainText(NOTE_B);
      const missingText = new RegExp([T('listFiles_missing'),T('availability_missing'), T('resourceState_missing'), T('issueKind_missing'), T('listState_missing'), T('availability_offline'), T('resourceState_offline')].map(escape).join('|'));
      await expect.soft(row, 'list entry says the local files are gone').toContainText(missingText, { timeout: 10_000 });
    });

    await test.step('重新加入來源並掃描 → 條目恢復正常狀態', async () => {
      await remap(TESTSET);
      await expect.poll(async () => (await core.api('/roots'))[0].missing, { timeout: 60_000 }).toBe(0);
      const panel = await openList(page, LIST);
      const row = entry(panel, offline.title);
      await expect(row).toContainText(NOTE_B);
      const state = (await core.api(`/lists/${(await core.api('/lists')).lists.find((l: any) => l.name === LIST).id}`)).entries.find((e: any) => e.local_work_id === offline.id);
      expect(state.reference_state).toBe('local');
      for (const key of ['availability_missing', 'resourceState_missing', 'listState_missing']) await expect(row).not.toContainText(T(key));
      await row.locator('strong button').click();
      await expect(page.locator('h1:visible').first()).toHaveText(offline.title);
    });
  });

  test('US-3.3 建立「年度最愛」→ 批次貼到 3 部 → 以標籤篩選正好 3 部；相似/重複名稱警告', async ({ page }) => {
    const chosen = await Promise.all(['v59027', 'v60462', 'v48419'].map(byVndb));
    const TAG = '年度最愛';
    await openApp(page, core);

    await test.step('建立標籤 → 選 3 部 → 批次貼上', async () => {
      const dialog = await tagDialog(page);
      await dialog.getByRole('button', { name: T('addCustomTag') }).click();
      await dialog.getByLabel(T('customTagName')).fill(TAG);
      await dialog.getByRole('button', { name: T('createTag') }).click();
      await expect(dialog.getByRole('heading', { name: T('focusTags') })).toBeVisible();
      await dialog.getByRole('button', { name: T('close') }).click();

      const { manager } = await manageTag(page, TAG);
      for (const w of chosen) {
        await manager.getByLabel(T('findWorks')).fill(w.title);
        await manager.locator('.custom-tag-works button').filter({ hasText: w.title }).first().click();
      }
      await expect(manager).toContainText(T('selectedTagWorks', { count: 3 }));
      await manager.getByRole('button', { name: T('tagAddSelected') }).click();
      await expect(manager.getByRole('status').filter({ hasText: T('tagBatchResult', { processed: 3, changed: 3, unchanged: 0, failed: 0 }) })).toBeVisible();
      await expect(manager).toContainText(T('tagCoverage', { count: 3, total: 3 }));
      await manager.getByLabel(T('findWorks')).fill('');
      for (const w of chosen) await expect(manager.locator('.custom-tag-works button').filter({ hasText: w.title }).first()).toHaveText(new RegExp(`${escape(w.title)}.*✓$`));
      const tag = (await tagsApi()).find(t => t.name === TAG);
      expect(await members(tag.id, (await works()).map(w => w.id))).toEqual(chosen.map(w => w.id).sort());
      await manager.getByRole('button', { name: T('done') }).click();
      await page.locator('dialog.focus-tag-dialog[open]').getByRole('button', { name: T('close') }).click();
    });

    await test.step('Collection 以「年度最愛」篩選 → 正好 3 部，計數正確', async () => {
      await filterByTag(page, TAG);
      await expect(cards(page)).toHaveCount(3);
      for (const w of chosen) await expect(card(page, w)).toHaveCount(1);
      await expect(page.getByText(T('filteredWorks', { total: (await works()).length, count: 3 })).first()).toBeVisible();
    });

    await test.step('名稱與既有標籤相似/重複時顯示警告', async () => {
      await createTagApi('Galroon Pick', []);
      await page.reload();
      const before = (await tagsApi()).length;
      const dialog = await tagDialog(page);
      await dialog.getByRole('button', { name: T('addCustomTag') }).click();
      const name = dialog.getByLabel(T('customTagName'));
      const warning = (names: string) => dialog.getByText(T('tagSimilarNames', { names }));

      await name.fill('galroon  pick'); // case and spacing differ from a custom tag
      await expect.soft(warning('Galroon Pick'), 'similar custom tag').toBeVisible();

      await name.fill('clone heroine'); // VNDB tag "Clone Heroine" on 宿りし乙女の誓いと魔法
      await expect.soft(dialog.getByText(T('tagSimilarVndb', { names: 'Clone Heroine' })), 'similar VNDB tag').toBeVisible({ timeout: 5_000 });

      await name.fill(TAG); // exact duplicate of a custom tag
      const warned = await dialog.getByRole('status').filter({ hasText: TAG }).isVisible();
      if (!warned) {
        await dialog.getByRole('button', { name: T('createTag') }).click();
        await expect.soft(dialog.getByRole('alert'), 'duplicate name is refused with a message').toBeVisible();
      }
      expect((await tagsApi()).length, 'no duplicate tag created').toBe(before);
    });
  });

  test('US-3.4 VNDB 標籤篩選結果正確，劇透標籤預設隱藏，隱藏不代表不存在', async ({ page }) => {
    const all = await works();
    const has = (tag: string) => all.filter(w => w.tags?.includes(tag));
    const spoilerWork = all.find(w => w.vndb_id === 'v55281')!; // 宿りし乙女の誓いと魔法
    await openApp(page, core);

    await test.step('選 NAS 測試集作品有的 VNDB 標籤 → 結果包含該作品，計數一致', async () => {
      // v55281 is tagged Trap Protagonist (not Male Protagonist) on VNDB; this tag is spoiler-free and shared by many works.
      const TAG = 'High School Student Heroine';
      const expected = has(TAG);
      expect(expected.length).toBeGreaterThan(1);
      expect(expected.map(w => w.id)).toContain(spoilerWork.id);
      const dialog = await tagDialog(page);
      await dialog.getByLabel(T('findTags')).fill(TAG);
      await expect(tagOption(dialog, TAG)).toHaveText(new RegExp(`${expected.length}$`));
      await tagOption(dialog, TAG).click();
      await dialog.getByRole('button', { name: T('done'), exact: true }).click();
      await expect(cards(page)).toHaveCount(expected.length);
      await expect(card(page, spoilerWork)).toHaveCount(1);
      await expect(page.getByText(T('filteredWorks', { total: all.length, count: expected.length })).first()).toBeVisible();
      await page.getByRole('button', { name: T('clearFilters') }).first().click();
    });

    await test.step('劇透標籤預設不顯示；開啟劇透顯示後才出現', async () => {
      // On VNDB, "Clone Heroine" is a major spoiler (level 2) for v55281; it is stored in the local catalog,
      // so the work page must hide it until spoilers are turned on.
      const SPOILER = 'Clone Heroine';
      expect(has(SPOILER).map(w => w.id)).toContain(spoilerWork.id);
      await searchCollection(page, spoilerWork.title);
      await openWork(page, spoilerWork);
      const tags = page.locator('details.work-tags:visible');
      await tags.locator('summary').click();
      await expect(tags.locator('.pill').first()).toBeVisible();
      await expect.soft(tags.locator('.pill', { hasText: new RegExp(`^${SPOILER}$`) }), 'spoiler tag hidden by default').toHaveCount(0);
      await page.getByRole('button', { name: T('noSpoilers') }).first().click();
      await expect(page.getByRole('button', { name: T('showingSpoilers') }).first()).toBeVisible();
      await expect(tags.locator('.pill', { hasText: new RegExp(`^${SPOILER}$`) })).toHaveCount(1);
    });

    await test.step('隱藏的劇透資料不讓篩選少算', async () => {
      // A major spoiler for v55281, but not hidden for every work that has it.
      const TAG = "Protagonist's Childhood Friend as a Heroine";
      const expected = has(TAG);
      expect(expected.map(w => w.id)).toContain(spoilerWork.id);
      await filterByTag(page, TAG);
      await expect(cards(page)).toHaveCount(expected.length);
      await expect(card(page, spoilerWork)).toHaveCount(1);
      await expect(page.getByText(T('filteredWorks', { total: all.length, count: expected.length })).first()).toBeVisible();
    });
  });

  test('US-3.5 標籤改名／合併／刪除先預覽影響範圍，確認後同步；取消不變更', async ({ page }) => {
    const [a, b, c, d] = await Promise.all(['v59027', 'v60462', 'v48419', 'v61834'].map(byVndb));
    const allIds = (await works()).map(w => w.id);
    const favorite = await createTagApi('年度最愛', [a.id, b.id, c.id]);
    const tagA = await createTagApi('A 組', [a.id, b.id]);
    const tagB = await createTagApi('B 組', [b.id, d.id]);
    const doomed = await createTagApi('待刪除', [a.id, c.id, d.id]);
    await openApp(page, core);

    await test.step('取消預覽 → 無任何變更', async () => {
      const { manager } = await manageTag(page, '年度最愛');
      await manager.getByRole('button', { name: T('tagRename') }).click();
      await manager.getByLabel(T('tagNewName')).fill('不要改');
      await manager.getByRole('button', { name: T('cancel') }).click();
      await manager.getByLabel(T('customTagName')).selectOption({ label: '待刪除' });
      await manager.getByRole('button', { name: T('tagDelete') }).click();
      await expect(manager.getByRole('region', { name: T('tagDeletePreview') })).toContainText(T('tagDeleteImpact', { name: '待刪除', count: 3 }));
      await manager.getByRole('button', { name: T('cancel') }).click();
      const tags = await tagsApi();
      expect(tags.find(t => t.id === favorite.id)).toMatchObject({ name: '年度最愛', deleted: false, work_count: 3, revision: 1 });
      expect(tags.find(t => t.id === doomed.id)).toMatchObject({ name: '待刪除', deleted: false, work_count: 3, revision: 1 });
      await manager.getByRole('button', { name: T('done') }).click();
      await page.locator('dialog.focus-tag-dialog[open]').getByRole('button', { name: T('close') }).click();
    });

    await test.step('改名「年度最愛」→「神作」：預覽影響 3 部 → 確認 → 3 部都是「神作」', async () => {
      const { manager } = await manageTag(page, '年度最愛');
      await manager.getByRole('button', { name: T('tagRename') }).click();
      await manager.getByLabel(T('tagNewName')).fill('神作');
      const editor = manager.locator('section').filter({ has: page.getByLabel(T('tagNewName')) });
      await expect.soft(editor, 'rename preview shows the 3 affected works').toContainText(/\b3\b/, { timeout: 3_000 });
      await manager.getByRole('button', { name: T('tagSaveName') }).click();
      await expect(manager.getByRole('status').filter({ hasText: T('tagEditSaved') })).toBeVisible();
      await manager.getByRole('button', { name: T('done') }).click();
      const dialog = page.locator('dialog.focus-tag-dialog[open]');
      await dialog.getByLabel(T('findTags')).fill('年度最愛');
      await expect(tagOption(dialog, '年度最愛')).toHaveCount(0);
      await dialog.getByRole('button', { name: T('close') }).click();
      await filterByTag(page, '神作');
      await expect(cards(page)).toHaveCount(3);
      for (const w of [a, b, c]) await expect(card(page, w)).toHaveCount(1);
      await expect(page.getByText(T('filteredWorks', { total: allIds.length, count: 3 })).first()).toBeVisible();
      await page.getByRole('button', { name: T('clearFilters') }).first().click();
    });

    await test.step('刪除標籤 → 預覽影響數 → 確認 → 作品不再有該標籤，作品本身不受影響', async () => {
      const { manager } = await manageTag(page, '待刪除');
      await manager.getByRole('button', { name: T('tagDelete') }).click();
      const preview = manager.getByRole('region', { name: T('tagDeletePreview') });
      await expect(preview).toContainText(T('tagDeleteImpact', { name: '待刪除', count: 3 }));
      await preview.getByRole('button', { name: T('tagConfirmDelete') }).click();
      await expect(manager.getByRole('status').filter({ hasText: T('tagEditSaved') })).toBeVisible();
      await manager.getByRole('button', { name: T('done') }).click();
      const dialog = page.locator('dialog.focus-tag-dialog[open]');
      await dialog.getByLabel(T('findTags')).fill('待刪除');
      await expect(tagOption(dialog, '待刪除')).toHaveCount(0);
      await dialog.getByRole('button', { name: T('close') }).click();
      expect((await core.api<any[]>('/custom-tags')).map(t => t.id)).not.toContain(doomed.id);
      const after = await works();
      expect(after.map(w => w.id).sort()).toEqual([...allIds].sort());
      for (const w of [a, c, d]) expect(after.find(x => x.id === w.id)).toMatchObject({ title: w.title, revision: w.revision });
      await searchCollection(page, '');
      await expect(page.locator('.collection-heading')).toContainText(String(allIds.length));
    });

    await test.step('合併 A 與 B → 作品都改為合併後標籤，同時有 A、B 的不重複', async () => {
      const { manager } = await manageTag(page, 'A 組');
      await manager.getByRole('button', { name: T('tagMerge') }).click();
      const preview = manager.getByRole('region', { name: T('tagMergePreview') });
      await preview.getByLabel(T('tagMergeInto')).selectOption({ label: 'B 組' });
      await expect(preview).toContainText(T('tagMergeImpact', { name: 'A 組', count: 2, target: 'B 組' }));
      await preview.getByRole('button', { name: T('tagConfirmMerge') }).click();
      await expect(manager.getByRole('status').filter({ hasText: T('tagEditSaved') })).toBeVisible();
      const live = (await core.api<any[]>('/custom-tags')).filter(t => t.name === 'B 組' || t.name === 'A 組');
      expect(live.map(t => t.name), 'one merged tag remains').toEqual(['B 組']);
      const merged = live[0];
      expect(await members(merged.id, allIds), 'merged membership').toEqual([a.id, b.id, d.id].sort());
      expect(merged.work_count, 'b counted once').toBe(3);
      void tagA; void tagB;
    });
  });
});

// =====================================================================
test.describe('US-2.5 大量目錄快照', () => {
  test.beforeEach(async () => { core = await startCore({ name: 'us3-lists-scale', from: `${SNAPSHOTS}/scale` }); });
  test.afterEach(async () => { await core?.stop(); });

  test('US-3.1 大量目錄：「加入篩選結果」涵蓋所有頁而非僅當頁', async ({ page }) => {
    const total = 3000, clover = 600; // 5 studios round-robin in the generated catalog
    const LIST = 'CloverGAME 全部';
    await openApp(page, core);
    await createList(page, LIST);
    await searchCollection(page, CLOVER);
    await expect(page.getByText(T('filteredWorks', { total, count: clover })).first()).toBeVisible();
    await expect(cards(page)).toHaveCount(60);
    await page.locator('.library-results .toolbar-menu summary').click();
    await page.getByRole('button', { name: T('listAddFiltered') }).click();
    const dialog = page.locator('dialog.add-to-list-dialog');
    await expect(dialog).toContainText(T('listScopeCount', { count: clover }), { timeout: 20_000 });
    await dialog.getByLabel(T('listDestination')).selectOption({ label: LIST });
    await dialog.getByRole('button', { name: T('listConfirmAdd') }).click();
    await expect(dialog.getByRole('status')).toHaveText(T('listAddedResult', { added: clover, skipped: 0 }), { timeout: 20_000 });
    await dialog.getByRole('button', { name: T('done') }).click();
    const panel = await openList(page, LIST);
    await expect(panel).toContainText(T('listCount', { count: clover }));
  });
});
