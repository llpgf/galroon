"""
Games API endpoints for Galgame Library Manager.

**PHASE 20.0: The Instant Index**
- Zero-latency reads: All queries hit SQLite, not filesystem
- FTS5 full-text search for instant text search
- SQL ORDER BY for instant sorting
- Background scanner keeps DB updated silently

**PHASE 19.6: Semantic Sanitization**
Renamed "play_status" to "library_status" to align with Asset Manager philosophy.

**PHASE 19.8: Performance Optimization (Lazy Loading)**
- NOTE: Lazy loading is now replaced by SQLite indexing
- Even faster than lazy loading (O(1) lookup instead of O(page_size))
"""

import logging
import json
from pathlib import Path
from typing import Dict, Any, List, Optional

from fastapi import APIRouter, HTTPException, Query, Request
from pydantic import BaseModel, Field

from ..config import get_config
from ..core.database import get_database
from ..metadata import get_resource_manager
from ..core.path_safety import is_safe_path

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/games", tags=["games"])


# ============================================================================
# Pydantic Models
# ============================================================================

class GameSummary(BaseModel):
    """Summary information for a game (used in list view)."""
    folder_path: str = Field(..., description="Game folder path (used as ID)")
    title: str = Field(..., description="Game title")
    developer: Optional[str] = Field(None, description="Developer/publisher name")
    cover_image: Optional[str] = Field(None, description="URL or path to cover image")
    badges: List[str] = Field(default_factory=list, description="Asset badges: ['ISO', 'DLC', 'Patch']")
    library_status: str = Field(default="unstarted", description="User's library status")
    rating: Optional[float] = Field(None, description="Rating score (0-10)")
    release_date: Optional[str] = Field(None, description="Release date (YYYY-MM-DD)")
    tags: List[str] = Field(default_factory=list, description="Provider tags")
    user_tags: List[str] = Field(default_factory=list, description="User-defined tags")


class GamesListResponse(BaseModel):
    """Response model for games list with pagination."""
    data: List[GameSummary]
    total: int = Field(..., description="Total number of games")
    page: int = Field(..., description="Current page number (1-indexed)")
    size: int = Field(..., description="Page size")
    strategy: str = Field(default="sqlite", description="Query strategy: always 'sqlite' now")


class LibraryStatusUpdateRequest(BaseModel):
    """Request model for updating library status."""
    library_status: str = Field(..., description="New library status")


class ScanResponse(BaseModel):
    """Response model for library scan."""
    status: str = Field(..., description="Scan status: 'scan_started' or 'already_scanning'")


class ScanStatsResponse(BaseModel):
    """Response model for scan statistics."""
    added: int = Field(..., description="Number of games added")
    modified: int = Field(..., description="Number of games modified")
    removed: int = Field(..., description="Number of games removed")
    total_time_ms: int = Field(..., description="Total scan time in milliseconds")
    status: str = Field(..., description="Scan status")


# ============================================================================
# API Endpoints
# ============================================================================

@router.get("", response_model=GamesListResponse)
@router.get("/", response_model=GamesListResponse)  # Handle trailing slash
async def list_games(
    request: Request,
    skip: int = Query(0, ge=0, description="Number of games to skip"),
    limit: int = Query(50, ge=1, le=200, description="Number of games to return"),
    sort_by: str = Query("recently_added", description="Sort by: recently_added, name, release_date, rating"),
    descending: bool = Query(True, description="Sort order"),
    search: Optional[str] = Query(None, description="Full-text search in title/developer/tags"),
    filter_tag: Optional[str] = Query(None, description="Filter by tag")
):
    """
    Get all games from the library (Phase 20.0: ZERO-LATENCY).

    **ZERO FILESYSTEM I/O** - All queries hit SQLite database!

    Performance:
    - List games: O(1) lookup in SQLite (instant)
    - Search: FTS5 full-text search (instant)
    - Sort: SQL ORDER BY with indexes (instant)
    - Filter: SQL WHERE with indexes (instant)

    The database is kept updated by the background scanner.

    Args:
        skip: Number of games to skip (pagination)
        limit: Number of games to return
        sort_by: Sort field
        descending: Sort order
        search: Full-text search query
        filter_tag: Filter by tag

    Returns:
        GamesListResponse with data from SQLite
    """
    db = get_database()

    # Query SQLite database (NO FILESYSTEM I/O!)
    games, total = db.get_games(
        skip=skip,
        limit=limit,
        sort_by=sort_by,
        descending=descending,
        search=search,
        filter_tag=filter_tag
    )

    # Convert DB rows to GameSummary
    summaries = []
    for game in games:
        summaries.append(GameSummary(
            folder_path=game['folder_path'],
            title=game['title'],
            developer=game.get('developer'),
            cover_image=game.get('cover_image'),
            badges=game.get('badges', []),
            library_status=game.get('library_status', 'unstarted'),
            rating=game.get('rating'),
            release_date=game.get('release_date'),
            tags=game.get('tags', []),
            user_tags=game.get('user_tags', [])
        ))

    return GamesListResponse(
        data=summaries,
        total=total,
        page=(skip // limit) + 1,
        size=limit,
        strategy="sqlite"  # Always SQLite now
    )


@router.get("/{game_id}", response_model=Dict[str, Any])
async def get_game_details(request: Request, game_id: str):
    """
    Get detailed information for a specific game.

    Args:
        game_id: Game folder path (used as identifier)

    Returns:
        Game details with full metadata
    """
    manager = get_resource_manager()
    config = get_config()

    # game_id is the folder path (relative or absolute)
    game_path = Path(game_id)

    # If relative path, try to find it in library roots
    if not game_path.is_absolute():
        for root in config.library_roots:
            candidate = root / game_id
            if candidate.exists():
                game_path = candidate
                break

    # Security check
    if not is_safe_path(game_path, config.library_root):
        raise HTTPException(
            status_code=400,
            detail=f"Invalid game path: {game_id}"
        )

    if not game_path.exists():
        raise HTTPException(
            status_code=404,
            detail=f"Game not found: {game_id}"
        )

    # Load metadata from file (detail view needs full metadata)
    meta_dict = manager.load_metadata(game_path)
    if not meta_dict:
        raise HTTPException(
            status_code=404,
            detail=f"Metadata not found for: {game_id}"
        )

    # Phase 19.6: Ensure legacy migration on read
    meta_dict = _migrate_legacy_status(meta_dict)

    # Add folder_path for frontend
    meta_dict['folder_path'] = str(game_path)

    return meta_dict


@router.patch("/{game_id}/status")
async def update_library_status(
    request: Request,
    game_id: str,
    payload: LibraryStatusUpdateRequest
):
    """
    Update the library status for a game.

    Phase 20.0: Updates BOTH metadata.json (portability) AND SQLite (instant cache).

    Phase 19.6: Semantic Sanitization - Renamed from "play_status"

    Args:
        game_id: Game folder path
        payload: LibraryStatusUpdateRequest with new library_status

    Returns:
        Success message
    """
    manager = get_resource_manager()
    config = get_config()
    db = get_database()

    # game_id is the folder path
    game_path = Path(game_id)

    # If relative path, try to find it in library roots
    if not game_path.is_absolute():
        for root in config.library_roots:
            candidate = root / game_id
            if candidate.exists():
                game_path = candidate
                break

    # Security check
    if not is_safe_path(game_path, config.library_root):
        raise HTTPException(
            status_code=400,
            detail=f"Invalid game path: {game_id}"
        )

    if not game_path.exists():
        raise HTTPException(
            status_code=404,
            detail=f"Game not found: {game_id}"
        )

    # Validate enum
    valid_statuses = ['unstarted', 'in_progress', 'finished', 'on_hold', 'dropped', 'planned']
    if payload.library_status not in valid_statuses:
        raise HTTPException(
            status_code=400,
            detail=f"Invalid library_status. Must be one of: {valid_statuses}"
        )

    # Load metadata
    meta_dict = manager.load_metadata(game_path)
    if not meta_dict:
        raise HTTPException(
            status_code=404,
            detail=f"Metadata not found for: {game_id}"
        )

    # Phase 19.6: Legacy migration before update
    meta_dict = _migrate_legacy_status(meta_dict)

    # Update library_status in metadata dict
    meta_dict['library_status'] = {
        'value': payload.library_status,
        'locked': False,
        'source': 'user'
    }

    # Save to metadata.json (portability)
    metadata_file = game_path / 'metadata.json'
    with open(metadata_file, 'w', encoding='utf-8') as f:
        json.dump(meta_dict, f, indent=2, ensure_ascii=False)

    # Phase 20.0: Also update SQLite (instant cache)
    json_mtime = metadata_file.stat().st_mtime
    folder_mtime = game_path.stat().st_mtime
    db.upsert_game(meta_dict, game_path, folder_mtime, json_mtime)

    logger.info(f"Updated library_status for {game_id} to {payload.library_status} (JSON + SQLite)")

    return {
        "success": True,
        "message": f"Library status updated to {payload.library_status}",
        "library_status": payload.library_status
    }


# ============================================================================
# Scanner Endpoints
# ============================================================================

@router.post("/scan", response_model=ScanResponse)
async def trigger_scan(request: Request):
    """
    Trigger a background library scan.

    Phase 20.0: Scanner runs in background thread, UI remains responsive.

    Algorithm:
    1. Fast diff: Compare filesystem vs database (in-memory set operations)
    2. Only read JSONs for modified games
    3. Auto-prune deleted folders

    Returns:
        ScanResponse with status
    """
    from ..services.scanner import get_scanner

    scanner = get_scanner()

    # Check if scan is already running
    if scanner.is_scanning():
        return ScanResponse(status="already_scanning")

    # Trigger background scan
    scanner.scan_library(background=True)

    return ScanResponse(status="scan_started")


@router.get("/scan/status", response_model=Dict[str, bool])
async def get_scan_status(request: Request):
    """
    Get current scan status.

    Returns:
        Dict with scanning status
    """
    from ..services.scanner import get_scanner

    scanner = get_scanner()

    return {
        "scanning": scanner.is_scanning()
    }


# ============================================================================
# Helper Functions (Legacy Support)
# ============================================================================

def extract_badges(metadata: Dict[str, Any]) -> List[str]:
    """
    Extract asset badges from metadata dictionary.

    Args:
        metadata: Metadata dictionary

    Returns:
        List of badge strings
    """
    badges = set()

    # Add detected assets
    assets_detected = metadata.get('assets_detected', [])
    if isinstance(assets_detected, list):
        for asset in assets_detected:
            asset_lower = asset.lower()
            if 'patch' in asset_lower:
                badges.add('Patch')
            elif 'dlc' in asset_lower or 'expansion' in asset_lower:
                badges.add('DLC')
            elif asset_lower.endswith('.iso') or asset_lower.endswith('.mdf'):
                badges.add('ISO')

    # Check versions for assets
    versions = metadata.get('versions', [])
    if isinstance(versions, list):
        for version in versions:
            if isinstance(version, dict):
                assets = version.get('assets', [])
                for asset in assets:
                    asset_lower = asset.lower()
                    if 'patch' in asset_lower:
                        badges.add('Patch')
                    elif 'dlc' in asset_lower or 'expansion' in asset_lower:
                        badges.add('DLC')
                    elif asset_lower.endswith('.iso'):
                        badges.add('ISO')

    return list(badges)


def _migrate_legacy_status(metadata_dict: Dict[str, Any]) -> Dict[str, Any]:
    """
    Phase 19.6: Migrate legacy play_status to library_status.

    Safe migration strategy:
    1. If library_status exists, do nothing
    2. If only play_status exists, migrate it to library_status
    3. Map old enum values to new enum values

    Args:
        metadata_dict: Metadata dictionary loaded from JSON

    Returns:
        Updated metadata dictionary
    """
    # Check if migration is needed
    if 'library_status' in metadata_dict:
        # Already migrated or using new field
        return metadata_dict

    if 'play_status' not in metadata_dict:
        # No legacy field to migrate
        return metadata_dict

    # Legacy migration: play_status → library_status
    legacy_status = metadata_dict['play_status']

    # Handle wrapped MetadataField
    if isinstance(legacy_status, dict):
        status_value = legacy_status.get('value')
    else:
        status_value = legacy_status

    if not status_value:
        return metadata_dict

    # Map old enum values to new enum values
    status_map = {
        'unplayed': 'unstarted',
        'playing': 'in_progress',
        'completed': 'finished',
        'paused': 'on_hold',
        'dropped': 'dropped',
        'wishlist': 'planned'
    }

    new_status = status_map.get(status_value, status_value)

    # Update metadata dictionary
    metadata_dict['library_status'] = {
        'value': new_status,
        'locked': legacy_status.get('locked', False) if isinstance(legacy_status, dict) else False,
        'source': 'migrated'
    }

    logger.info(f"Migrated legacy status: {status_value} → {new_status}")

    # Remove old play_status field
    del metadata_dict['play_status']

    return metadata_dict


def metadata_to_summary(metadata_dict: Dict[str, Any], folder_path: str) -> GameSummary:
    """
    Convert metadata dictionary to GameSummary.

    NOTE: Phase 20.0 - This is now only used for detail view.
    List view uses SQLite directly.

    Args:
        metadata_dict: Metadata dictionary
        folder_path: Game folder path

    Returns:
        GameSummary object
    """
    # Extract title with fallback
    title_obj = metadata_dict.get('title', {})
    if isinstance(title_obj, dict) and 'value' in title_obj:
        title_value = title_obj['value']
        if isinstance(title_value, dict):
            title = (title_value.get('zh_hant') or title_value.get('zh_hans') or
                    title_value.get('en') or title_value.get('ja') or
                    title_value.get('original') or 'Untitled')
        else:
            title = str(title_value) if title_value else 'Untitled'
    else:
        title = folder_path.name

    # Extract developer
    developer_obj = metadata_dict.get('developer')
    if isinstance(developer_obj, dict) and 'value' in developer_obj:
        developer = developer_obj['value']
    else:
        developer = None

    # Extract cover image
    cover_image = metadata_dict.get('cover_path') or metadata_dict.get('cover_url', {})
    if isinstance(cover_image, dict) and 'value' in cover_image:
        cover_image = cover_image['value']

    # Extract library status
    library_status_obj = metadata_dict.get('library_status')
    if isinstance(library_status_obj, dict) and 'value' in library_status_obj:
        library_status = library_status_obj['value']
    elif isinstance(library_status_obj, str):
        library_status = library_status_obj
    else:
        library_status = 'unstarted'

    # Extract rating
    rating_obj = metadata_dict.get('rating')
    if isinstance(rating_obj, dict) and 'value' in rating_obj:
        value = rating_obj['value']
        if isinstance(value, dict) and 'score' in value:
            rating = value['score']
        else:
            rating = None
    else:
        rating = None

    # Extract release date
    release_date_obj = metadata_dict.get('release_date')
    if isinstance(release_date_obj, dict) and 'value' in release_date_obj:
        release_date = release_date_obj['value']
    else:
        release_date = None

    # Extract badges
    badges = extract_badges(metadata_dict)

    # Extract tags
    tags_obj = metadata_dict.get('tags')
    if isinstance(tags_obj, dict) and 'value' in tags_obj:
        tags = tags_obj['value']
    elif isinstance(tags_obj, list):
        tags = tags_obj
    else:
        tags = []

    user_tags = metadata_dict.get('user_tags', [])
    if not isinstance(user_tags, list):
        user_tags = []

    return GameSummary(
        folder_path=str(folder_path),
        title=title,
        developer=developer,
        cover_image=cover_image,
        badges=badges,
        library_status=library_status,
        rating=rating,
        release_date=release_date,
        tags=tags,
        user_tags=user_tags
    )
