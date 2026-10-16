// US-4.1 安裝 → 首次開啟 → 加來源 → 出卡（W）. Runs on the Windows test laptop against the installed app,
// connected over WebView2 remote debugging (scripts/windows/acceptance.ps1 launches it with a fresh GALROON_DATA).
import { test, expect, chromium, type Browser, type Page } from '@playwright/test';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmdirSync, unlinkSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { T, nav } from '../e2e/support/app';

const CDP = process.env.GALROON_CDP || 'http://127.0.0.1:9222';
const SOURCE = process.env.GALROON_WIN_SOURCE || 'D:\\galroon-mcp-sandbox\\source';
// A folder the watcher must pick up without any click: a stand-in archive named after a real work.
const FIXTURE = '[260529] [Whirlpool] Relirium -レリリウム- 遺跡と出逢いと冒険と 通常版';
const FIXTURE_WORK = 'v61834';

type Want = { work?: string; works?: string[]; manual?: boolean; unmatched?: boolean; split?: Record<string, Want> };
// The Windows source is a 4-folder subset of the retired sandbox_data set, so its answers come from that key.
const KEY: Record<string, Want> = JSON.parse(readFileSync(fileURLToPath(new URL('../e2e/golden/sandbox_data.json', import.meta.url)), 'utf8')).folders;
const nfc = (s: string) => s.normalize('NFC');
KEY[FIXTURE] = { work: FIXTURE_WORK };

/** Calls Core the way the app does: the session comes from the desktop shell. */
async function api<T = any>(page: Page, path: string, body?: unknown): Promise<T> {
  return page.evaluate(async ({ path, body }) => {
    const s = await (window as any).__TAURI_INTERNALS__.invoke('core_session');
    const base = s.proxy_url || `${s.url}/api`;
    const token = s.proxy_url ? s.proxy_key : s.token;
    const r = await fetch(`${base}${path}`, { method: body === undefined ? 'GET' : 'POST', headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) });
    if (!r.ok) throw new Error(`${path} → ${r.status}: ${await r.text()}`);
    const text = await r.text();
    return text ? JSON.parse(text) : undefined;
  }, { path, body });
}

/** Every VNDB id bound to a resource, per top-level folder, compared with the answer key. */
async function grade(page: Page) {
  const resources: any[] = await api(page, '/resources');
  const works: any[] = await api(page, '/works');
  const vndb = new Map(works.map(w => [w.id, w.vndb_id || `manual:${w.title}`]));
  const falseMatches: string[] = [], matched = new Set<string>();
  for (const r of resources.filter(r => r.files > 0)) {
    const path = nfc(r.path.replaceAll('\\', '/'));
    const top = path.split('/')[0];
    const want = Object.entries(KEY).find(([k]) => nfc(k) === top)?.[1];
    if (!want) continue;
    const member = want.split ? Object.entries(want.split).find(([m]) => nfc(`${top}/${m}`) === path)?.[1] : want;
    const allowed = new Set(member ? (member.works || (member.work ? [member.work] : [])) : []);
    for (const id of new Set([...(r.bindings || []).map((b: any) => b.work_id), ...(r.work_id ? [r.work_id] : [])])) {
      const got = vndb.get(id) || `unknown:${id}`;
      if (!allowed.has(got) || member?.unmatched) falseMatches.push(`${path} → ${got}`); else matched.add(got);
    }
  }
  return { falseMatches, matched, works };
}

/** Works that automatic matching must produce for the folders actually present in the source. */
function expectedWorks(): string[] {
  const out = new Set<string>();
  for (const name of readdirSync(SOURCE).filter(n => nfc(n) !== nfc(FIXTURE))) {
    const want = Object.entries(KEY).find(([k]) => nfc(k) === nfc(name))?.[1];
    for (const e of want ? (want.split ? Object.values(want.split) : [want]) : []) if (!e.manual && !e.unmatched) for (const id of e.works || (e.work ? [e.work] : [])) out.add(id);
  }
  return [...out];
}

let browser: Browser, page: Page;
const fixtureDir = join(SOURCE, FIXTURE);

test.beforeAll(async () => {
  // A leftover fixture would be found by the first scan and make the watcher step prove nothing.
  expect(existsSync(fixtureDir), `remove the leftover fixture first: ${fixtureDir}`).toBe(false);
  browser = await chromium.connectOverCDP(CDP);
  page = browser.contexts().flatMap(c => c.pages()).find(p => !p.url().startsWith('devtools:'))!;
  expect(page, 'the installed app window').toBeTruthy();
  await page.evaluate(() => localStorage.setItem('galroon.language', 'zh-TW'));
  await page.reload();
});

/** Removes only the fixture this test created (its archive holds a fixed marker text). */
function removeFixture() {
  // Not rmSync({ recursive }): Node 24's native recursive delete aborts the process on this Japanese path on Windows.
  const archive = join(fixtureDir, `${FIXTURE}.rar`);
  try { if (readFileSync(archive, 'utf8') === 'galroon acceptance fixture') { unlinkSync(archive); rmdirSync(fixtureDir); } } catch { /* not created */ }
}

test('US-4.1 安裝後首次開啟 → 加來源 → 自動出封面卡 → 監看新資料夾', async () => {
  try {
    await test.step('首次開啟自動連上本機 Core，沒有錯誤畫面', async () => {
      await expect(page.getByRole('navigation')).toBeVisible({ timeout: 60_000 });
      expect((await api(page, '/health')).name).toBe('Galroon');
      await expect(page.locator('main [role=alert]:visible')).toHaveCount(0);
    });

    await test.step('Sources 加入測試來源並儲存', async () => {
      await nav(page, 'sources');
      await page.getByRole('button', { name: T('addSource') }).first().click();
      await page.getByLabel(T('sourceLabel')).fill('windows-acceptance');
      await page.getByLabel(T('sourcePath')).fill(SOURCE);
      await page.getByRole('button', { name: T('save'), exact: true }).click();
      await expect(page.getByText(SOURCE)).toBeVisible();
    });

    await test.step('不按任何按鈕：自動掃描與配對完成', async () => {
      await expect.poll(async () => { const m = await api(page, '/matching'); return m.job?.state === 'completed' && m.job.processed === m.job.total; }, { timeout: 10 * 60_000, intervals: [3_000] }).toBe(true);
    });

    await test.step('錯配 = 0，每部應配作品都出現', async () => {
      const { falseMatches, matched } = await grade(page);
      expect(falseMatches, 'false matches').toEqual([]);
      expect(expectedWorks().filter(id => !matched.has(id)), 'expected works without a card').toEqual([]);
    });

    await test.step('Collection 出現帶封面的卡', async () => {
      await nav(page, 'library');
      const works: any[] = await api(page, '/works');
      const cards = page.locator('.book-grid .book');
      await expect(cards).toHaveCount(works.length);
      const withCover = works.filter(w => w.cover).length;
      expect(withCover, 'works with a cover URL').toBe(works.length);
      await expect.poll(() => page.locator('.book-grid .book img').evaluateAll(imgs => imgs.filter(i => (i as HTMLImageElement).naturalWidth > 0).length), { timeout: 120_000, message: 'covers loaded' }).toBe(withCover);
    });

    await test.step('原生檔案監視：複製新資料夾進來源 → 不按按鈕出現新卡', async () => {
      mkdirSync(fixtureDir, { recursive: true });
      writeFileSync(join(fixtureDir, `${FIXTURE}.rar`), 'galroon acceptance fixture');
      await expect.poll(async () => (await api<any[]>(page, '/works')).some(w => w.vndb_id === FIXTURE_WORK), { timeout: 5 * 60_000, intervals: [5_000] }).toBe(true);
      const { falseMatches } = await grade(page);
      expect(falseMatches, 'false matches after the watcher run').toEqual([]);
      await nav(page, 'library');
      const works: any[] = await api(page, '/works');
      await expect(page.locator('.book-grid .book')).toHaveCount(works.length);
    });
  } finally {
    removeFixture();
    // Detach from WebView2 inside the test: a CDP socket still open at worker teardown crashes Node on Windows.
    await browser.close();
  }
});
