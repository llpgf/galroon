import { afterEach, expect, test, vi } from 'vitest';
import { expectedCards, expectedRows, GOLDEN, grade, type Resource, type Work } from '../e2e/support/golden';
import type { Core } from '../e2e/support/core';

const compilation = Object.keys(GOLDEN.folders).find(folder => GOLDEN.folders[folder].works)!;
const patch = Object.keys(GOLDEN.folders).find(folder => GOLDEN.folders[folder].unmatched)!;
const works: Work[] = [
  { id: 'first', vndb_id: 'v1216', title: 'First', revision: 1 },
  { id: 'second', vndb_id: 'v1270', title: 'Second', revision: 1 },
  { id: 'other', vndb_id: 'v97', title: 'Other', revision: 1 },
  { id: 'manual', title: 'Manual card', revision: 1 },
];

async function row(path: string, ids: string[], primary: string | null = null) {
  const resources: Resource[] = [{ id: 'resource', path, files: 1, work_id: primary,
    bindings: ids.map(work_id => ({ work_id, release_id: null, role: 'main' })) }];
  const core = { api: vi.fn(async (endpoint: string) => endpoint === '/resources' ? resources : works) } as unknown as Core;
  return (await grade(core)).find(r => (r.member ? `${r.folder}/${r.member}` : r.folder) === path.normalize('NFC'))!;
}

test.each([
  [['second', 'first'], 'ok'],
  [['first', 'second', 'first'], 'ok'],
  [['first'], 'unmatched'],
  [[], 'unmatched'],
  [['first', 'second', 'other'], 'FALSE MATCH'],
  [['first', 'manual'], 'FALSE MATCH'],
  [['first', 'missing-work'], 'FALSE MATCH'],
] as const)('compilation bindings %j grade as %s', async (ids, verdict) => {
  expect(await row(compilation, [...ids])).toMatchObject({ expected: 'v1216, v1270', verdict });
});

test('a compilation cannot fill in a missing binding from the primary work', async () => {
  expect((await row(compilation, ['first'], 'second')).verdict).toBe('unmatched');
});

test('unmatched folders reject any binding, including a secondary or unresolved work', async () => {
  expect((await row(patch, [])).verdict).toBe('ok');
  for (const ids of [['other'], ['manual'], ['missing-work']]) {
    expect((await row(patch, ids)).verdict).toBe('FALSE MATCH');
  }
  expect((await row(patch, [], 'other')).verdict).toBe('FALSE MATCH');
});

test('manual split members allow no binding or the correct work, but reject other works', async () => {
  const member = Object.keys(GOLDEN.folders.WaffleTokuten.split!).find(member => GOLDEN.folders.WaffleTokuten.split![member].manual)!;
  const path = `WaffleTokuten/${member}`;
  expect((await row(path, [])).verdict).toBe('ok');
  expect((await row(path, ['other'])).verdict).toBe('FALSE MATCH');
  const manualFolder = Object.keys(GOLDEN.folders).find(folder => GOLDEN.folders[folder].manual)!;
  const work: Work = { id: 'correct', vndb_id: GOLDEN.folders[manualFolder].work, title: 'Correct', revision: 1 };
  works.push(work);
  try { expect((await row(manualFolder, ['correct'])).verdict).toBe('ok'); }
  finally { works.pop(); }
});

test('ordinary folders reject extra bindings and normalize decomposed paths', async () => {
  const folder = Object.keys(GOLDEN.folders).find(folder => GOLDEN.folders[folder].work === 'v97')!;
  expect((await row(folder.normalize('NFD'), ['other'])).verdict).toBe('ok');
  expect((await row(folder, ['other', 'first'], 'other')).verdict).toBe('FALSE MATCH');
});

test('row and card counts include split members and compilation works, deduplicating copies', () => {
  expect(expectedRows()).toBe(44);
  expect(expectedRows({ includeManual: false })).toBe(41);
  // Two compilation works, duplicate SPARE copies, and repeated bonus members count once per card.
  expect(expectedCards()).toBe(37);
});

afterEach(() => { vi.unstubAllEnvs(); vi.resetModules(); });

test.each([undefined, '/galroon-testset-does-not-exist', new URL(import.meta.url).pathname])('TESTSET rejects a missing/non-directory location: %s', async path => {
  vi.stubEnv('GALROON_TESTSET', path);
  await expect(import('../e2e/support/core')).rejects.toThrow('GALROON_TESTSET must point at the NAS test set copy (see docs/plans/2026-10-16-e2e-nas-test-set.md)');
});

test('TESTSET uses the required environment directory', async () => {
  const dir = new URL('.', import.meta.url).pathname;
  vi.stubEnv('GALROON_TESTSET', dir);
  expect((await import('../e2e/support/core')).TESTSET).toBe(dir);
});
