// Tauri IPC hooks for the frontend.
// Communicates with Rust backend via invoke().

import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';

export interface WorkSummary {
      id: string;
      title: string;
      cover_path: string | null;
      developer: string | null;
      rating: number | null;
      library_status: string;
      enrichment_state: string;
      tags: string[];
      release_date: string | null;
      vndb_id: string | null;
      bangumi_id: string | null;
      dlsite_id: string | null;
      variant_count: number;
      asset_count: number;
      asset_types: string[];
      primary_asset_type: string | null;
}

export interface Work {
      id: string;
      folder_path: string;
      title: string;
      title_original: string | null;
      title_aliases: string[];
      developer: string | null;
      publisher: string | null;
      release_date: string | null;
      rating: number | null;
      vote_count: number | null;
      description: string | null;
      cover_path: string | null;
      tags: string[];
      user_tags: string[];
      library_status: string;
      vndb_id: string | null;
      bangumi_id: string | null;
      dlsite_id: string | null;
      enrichment_state: string;
      title_source: string;
      field_sources: Record<string, string>;
      field_preferences: Record<string, string>;
      user_overrides: Record<string, unknown>;
}

export interface ScanResult {
      job_id: number | null;
      state: string;
      added: number;
      removed: number;
      modified: number;
      moved: number;
      total: number;
}

export type SortField = 'title' | 'rating' | 'release_date' | 'created_at';
export type SortDirection = 'asc' | 'desc';

interface ListWorksResponse {
      data: WorkSummary[];
      total: number;
      page: number;
      size: number;
}

export async function listWorks(
      page: number = 1,
      size: number = 50,
      assetType: string | null = null,
): Promise<WorkSummary[]> {
      const resp = await invoke<ListWorksResponse>('list_works', {
            page,
            size,
            assetType,
      });
      return resp.data ?? [];
}

export async function getWork(id: string): Promise<Work> {
      return invoke<Work>('get_work', { id });
}

export async function updateWork(
      id: string,
      updates: Partial<Work>
): Promise<void> {
      return invoke<void>('update_work', { id, updates });
}

export async function triggerScan(): Promise<ScanResult> {
      return invoke<ScanResult>('trigger_scan');
}

export async function searchWorks(query: string): Promise<WorkSummary[]> {
      const results = await invoke<WorkSummary[]>('search_works', { query });
      return Array.isArray(results) ? results : [];
}

export function toAssetUrl(path: string | null): string | null {
      if (!path) return null;
      if (/^https?:\/\//i.test(path)) return path;
      return convertFileSrc(path);
}

export function formatRating(rating: number | null): string {
      if (rating === null || rating === undefined) return '—';
      return rating.toFixed(1);
}

export function enrichmentLabel(state: string): string {
      const labels: Record<string, string> = {
            unmatched: 'Unmatched',
            pending_review: 'Review',
            matched: 'Matched',
            rejected: 'Rejected',
      };
      return labels[state] || state;
}

export function statusLabel(status: string): string {
      const labels: Record<string, string> = {
            unplayed: 'Unplayed',
            playing: 'Playing',
            completed: 'Completed',
            on_hold: 'On Hold',
            dropped: 'Dropped',
            wishlist: 'Wishlist',
      };
      return labels[status] || status;
}

export function statusColor(status: string): string {
      const colors: Record<string, string> = {
            unplayed: 'var(--text-muted)',
            playing: 'var(--accent-primary)',
            completed: 'var(--success)',
            on_hold: '#ffa94d',
            dropped: 'var(--danger)',
            wishlist: 'var(--accent-secondary)',
      };
      return colors[status] || 'var(--text-muted)';
}
