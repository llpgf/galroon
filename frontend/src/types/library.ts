/**
 * Sprint 6: The View Layer - Library DTOs
 *
 * Type definitions for library entries from backend CQRS Lite read model.
 *
 * Design Principles:
 * - Single Truth: UI state driven solely by entry_type from backend
 * - No Inference: Frontend must NOT infer entry_type from other fields
 * - UI Isolation: Raw scanner paths never exposed unless orphan
 */

export type LibraryEntryType = 'canonical' | 'suggested' | 'orphan';

export interface LibraryEntryDTO {
  view_id: string;
  entry_type: LibraryEntryType;
  display_title: string;
  cover_image?: string | null;
  metadata?: string | null;
  cluster_id?: string | null;
  canonical_id?: string | null;
  instance_count?: number | null;
  confidence_score?: number | null;
  created_at?: string;
}
