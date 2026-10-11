// Compares Core's catalog with the user-reviewed answer key (tests/e2e/golden/sandbox_data.json).
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Core } from './core';

type Expect = { work?: string; unmatched?: boolean; remake_release?: string; card_title?: string; card_released?: string; same_edition_as?: string; bonus_only?: boolean; split?: Record<string, Expect> };
export const GOLDEN: { folders: Record<string, Expect> } = JSON.parse(readFileSync(fileURLToPath(new URL('../golden/sandbox_data.json', import.meta.url)), 'utf8'));

export type Resource = { id: string; path: string; files: number; work_id?: string | null; release?: string; bindings: { work_id: string; release_id: string | null; role: string }[] };
export type Work = { id: string; vndb_id?: string | null; title: string; original_title?: string | null; released?: string | null; developer?: string | null; cover?: string | null; resources?: number };
export type Row = { folder: string; member?: string; expected: string; actual: string; verdict: 'ok' | 'unmatched' | 'FALSE MATCH' | 'missing' };

export async function catalog(core: Core) {
  const resources = await core.api<Resource[]>('/resources');
  const works = await core.api<Work[]>('/works');
  return { resources, works, byId: new Map(works.map(w => [w.id, w])) };
}

/** Grades every golden folder (and split member). False matches are hard failures. */
export async function grade(core: Core) {
  const { resources: all, byId } = await catalog(core);
  // A resource left without files (e.g. a folder later split into members) is not part of the collection.
  const resources = all.filter(r => r.files > 0);
  const rows: Row[] = [];
  const judge = (folder: string, member: string | undefined, want: Expect, found: Resource[]) => {
    if (found.length === 0) { rows.push({ folder, member, expected: want.work || 'unmatched', actual: '(no resource)', verdict: 'missing' }); return; }
    for (const r of found) {
      const work = r.work_id ? byId.get(r.work_id) : undefined;
      const actual = work?.vndb_id || (work ? `manual:${work.title}` : 'unmatched');
      const verdict = !work ? (want.unmatched ? 'ok' : 'unmatched') : actual === want.work ? 'ok' : 'FALSE MATCH';
      rows.push({ folder, member, expected: want.work || 'unmatched', actual, verdict });
    }
  };
  for (const [folder, want] of Object.entries(GOLDEN.folders)) {
    if (want.split) {
      for (const [member, m] of Object.entries(want.split)) judge(folder, member, m, resources.filter(r => r.path === `${folder}/${member}` || r.path === `${folder}\\${member}`));
      const whole = resources.filter(r => r.path === folder);
      if (whole.length) rows.push({ folder, expected: 'split into members', actual: 'single resource', verdict: whole.some(r => r.work_id) ? 'FALSE MATCH' : 'unmatched' });
    } else judge(folder, undefined, want, resources.filter(r => r.path === folder));
  }
  return rows;
}

export function summary(rows: Row[]) {
  const count = (v: Row['verdict']) => rows.filter(r => r.verdict === v).length;
  const table = rows.map(r => `${r.verdict.padEnd(11)} ${(r.member ? `${r.folder} / ${r.member}` : r.folder).slice(0, 90).padEnd(92)} want ${r.expected.padEnd(9)} got ${r.actual}`).join('\n');
  return { ok: count('ok'), unmatched: count('unmatched') + count('missing'), falseMatches: count('FALSE MATCH'), table };
}

/** Expected number of Collection cards: one per distinct work, remakes count separately from originals. */
export function expectedCards() {
  const keys = new Set<string>();
  for (const want of Object.values(GOLDEN.folders)) {
    for (const e of want.split ? Object.values(want.split) : [want]) if (e.work) keys.add(e.remake_release || e.work);
  }
  return keys.size;
}

/** Name, size and mtime of every file under a folder: proof that read-only flows changed nothing. */
export function fingerprint(root: string) {
  const out: string[] = [];
  const walk = (dir: string) => { for (const name of readdirSync(dir).sort()) { const p = join(dir, name); const s = statSync(p); if (s.isDirectory()) walk(p); else out.push(`${p.slice(root.length)}\t${s.size}\t${s.mtimeMs}`); } };
  walk(root);
  return out;
}
