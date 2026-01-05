"""
Library API for Galroon v0.2.0

Sprint 5: CQRS Lite - Read View (Query API)

Architecture:
- Queries library_entry_view SQL View
- Aggregates Canonical, Suggested, Orphan entries
- Enforces UI ISOLATION (no raw paths exposed to frontend)

This implements CQRS Lite: Separate Query API from Command API (decisions.py).
"""

import logging
import json
from typing import List, Optional

from fastapi import APIRouter, Query, Depends

from ...core.database import get_database  # 3 dots: app/api/v1 -> app/api -> app -> app/core
from .dto import LibraryEntry, LibraryListResponse, LibraryEntryType

logger = logging.getLogger(__name__)

# Create router (prefix added by parent api_v1_router)
router = APIRouter(tags=["library"])


# ============================================================================
# Library Query Endpoints
# ============================================================================

@router.get("/library", response_model=LibraryListResponse)
async def get_library(
    skip: int = Query(0, ge=0, description="Number of entries to skip"),
    limit: int = Query(50, ge=1, le=200, description="Number of entries to return"),
    entry_type: Optional[str] = Query(None, description="Filter by type: canonical, suggested, orphan"),
    db = Depends(get_database)
):
    """
    Get library entries from library_entry_view.

    **UI ISOLATION**: NO raw scanner paths exposed to frontend.
    - Canonical entries: Show canonical.display_title
    - Suggested entries: Show cluster.suggested_title
    - Orphan entries: Show title (can be folder name)

    Args:
        skip: Number of entries to skip (pagination)
        limit: Number of entries to return
        entry_type: Optional filter by entry type

    Returns:
        LibraryListResponse with UI-safe entries
    """
    logger.info(f"Fetching library: skip={skip}, limit={limit}, type={entry_type}")

    # Build query
    query = "SELECT * FROM library_entry_view"
    params = []

    # Filter by entry type
    if entry_type:
        if entry_type in ['canonical', 'suggested', 'orphan']:
            query += " WHERE entry_type = ?"
            params.append(entry_type)
        else:
            # Invalid entry_type, ignore filter
            pass

    # Order by display title
    query += " ORDER BY display_title COLLATE NOCASE"

    # Get total count
    with db.get_cursor() as cursor:
        if entry_type:
            cursor.execute(f"SELECT COUNT(*) FROM ({query})", params)
        else:
            cursor.execute("SELECT COUNT(*) FROM library_entry_view")
        total = cursor.fetchone()[0]

        # Apply pagination
        query += " LIMIT ? OFFSET ?"
        params.extend([limit, skip])

        cursor.execute(query, params)
        rows = cursor.fetchall()

    # Convert rows to LibraryEntry DTOs
    entries = []
    for row in rows:
        entry = _row_to_library_entry(row)
        entries.append(entry)

    return LibraryListResponse(
        entries=entries,
        total=total
    )


@router.get("/library/summary")
async def get_library_summary(
    db = Depends(get_database)
):
    """
    Get library summary statistics.

    Returns counts by entry type:
    - canonical: Number of canonical entities
    - suggested: Number of pending clusters
    - orphan: Number of orphaned instances
    """
    summary = {
        'canonical': 0,
        'suggested': 0,
        'orphan': 0,
        'total': 0
    }

    with db.get_cursor() as cursor:
        # Count by entry type
        cursor.execute("""
            SELECT entry_type, COUNT(*) as count
            FROM library_entry_view
            GROUP BY entry_type
        """)

        for row in cursor.fetchall():
            # Convert sqlite3.Row to dict for safe access
            row_dict = dict(row)
            entry_type = row_dict['entry_type']
            count = row_dict['count']
            summary[entry_type] = count
            summary['total'] += count

    return summary


# ============================================================================
# Helper Functions
# ============================================================================

def _row_to_library_entry(row) -> LibraryEntry:
    """
    Convert database row to LibraryEntry DTO.

    Enforces UI ISOLATION:
    - No raw folder paths in display_title (unless orphan)
    - Metadata is UI-safe (no filesystem info)
    """
    # Convert sqlite3.Row to dict for safe attribute access
    row_dict = dict(row)
    
    # Parse metadata JSON
    metadata_str = row_dict.get('metadata') or '{}'
    try:
        metadata = json.loads(metadata_str)
    except (json.JSONDecodeError, TypeError):
        metadata = {}

    return LibraryEntry(
        entry_id=row_dict['entry_id'],
        entry_type=row_dict['entry_type'],
        display_title=row_dict['display_title'],
        cover_image_url=row_dict.get('cover_image_url'),
        metadata=metadata,
        cluster_id=row_dict.get('cluster_id'),
        canonical_id=row_dict.get('canonical_id'),
        instance_count=row_dict.get('instance_count', 1),
        confidence_score=row_dict.get('confidence_score'),
        created_at=row_dict.get('created_at')
    )
