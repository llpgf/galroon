/**
 * API Client for Galroon Backend
 * 
 * All API calls to the backend are made through this module.
 * Uses axios for HTTP requests with proper error handling.
 */

import apiClient from './api/client';

// Types matching backend DTOs
export interface LibraryEntry {
      entry_id: string;
      entry_type: 'canonical' | 'suggested' | 'orphan';
      display_title: string;
      cover_image_url?: string;
      metadata: Record<string, unknown>;
      cluster_id?: string;
      canonical_id?: string;
      instance_count: number;
      confidence_score?: number;
      created_at?: string;
}

export interface LibraryListResponse {
      entries: LibraryEntry[];
      total: number;
}

export interface LibrarySummary {
      canonical: number;
      suggested: number;
      orphan: number;
      total: number;
}

export interface ClusterDetail {
      cluster_id: string;
      status: 'suggested' | 'accepted' | 'rejected';
      suggested_title: string;
      confidence_score: number;
      suggested_canonical_id?: string;
      instances: Array<Record<string, unknown>>;
      metadata?: Record<string, unknown>;
      created_at?: string;
}

// ============================================================================
// Library API
// ============================================================================

/**
 * Get library entries with pagination and optional type filter
 */
export async function getLibrary(
      skip = 0,
      limit = 50,
      entryType?: 'canonical' | 'suggested' | 'orphan'
): Promise<LibraryListResponse> {
      const params: Record<string, unknown> = { skip, limit };
      if (entryType) params.entry_type = entryType;

      const response = await apiClient.get<LibraryListResponse>('/api/v1/library', { params });
      return response.data;
}

/**
 * Get library summary (counts by type)
 */
export async function getLibrarySummary(): Promise<LibrarySummary> {
      const response = await apiClient.get<LibrarySummary>('/api/v1/library/summary');
      return response.data;
}

// ============================================================================
// Cluster/Decision API
// ============================================================================

/**
 * Get cluster details by ID
 */
export async function getClusterDetail(clusterId: string): Promise<ClusterDetail> {
      const response = await apiClient.get<ClusterDetail>(`/api/v1/decisions/clusters/${clusterId}`);
      return response.data;
}

/**
 * Accept a cluster (promote to canonical)
 */
export async function acceptCluster(
      clusterId: string,
      customTitle?: string
): Promise<{ canonical_id: string }> {
      const response = await apiClient.post(`/api/v1/decisions/clusters/${clusterId}/decide`, {
            decision: 'accept',
            custom_title: customTitle,
      });
      return response.data;
}

/**
 * Reject a cluster (dissolve, leave as orphans)
 */
export async function rejectCluster(clusterId: string): Promise<void> {
      await apiClient.post(`/api/v1/decisions/clusters/${clusterId}/decide`, {
            decision: 'reject',
      });
}

// ============================================================================
// Tags API (if available)
// ============================================================================

export interface Tag {
      id: string;
      name: string;
      game_count: number;
}

/**
 * Get all tags
 */
export async function getTags(): Promise<Tag[]> {
      const response = await apiClient.get<Tag[]>('/api/v1/tags');
      return response.data;
}

// Export the axios instance for custom requests
export default apiClient;
