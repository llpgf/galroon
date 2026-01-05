"""
Connectors API endpoints for Galgame Library Manager.

**PHASE 12: The Connector - API Endpoints**

Provides REST API for external integrations:
- Sync game with external providers
- Check connector status
- Manual enrichment triggers
"""

import logging
from pathlib import Path
from typing import Dict, Any, List, Optional

from fastapi import APIRouter, HTTPException, status, Query
from pydantic import BaseModel, Field

from ..config import get_config
from ..metadata.enricher import get_enricher, EnrichmentResult
from ..metadata.manager import get_resource_manager
from ..metadata.models import UnifiedMetadata

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/connectors", tags=["connectors"])


# ============================================================================
# Pydantic Models
# ============================================================================

class SyncRequest(BaseModel):
    """Request model for syncing a game."""
    game_path: str = Field(..., description="Path to game folder (relative to library root)")
    force_steam: bool = Field(False, description="Force Steam enrichment")
    force_bangumi: bool = Field(False, description="Force Bangumi enrichment")
    download_assets: bool = Field(True, description="Download assets or just URLs")


class SyncResponse(BaseModel):
    """Response model for sync result."""
    success: bool
    game_path: str
    steam_id: Optional[str] = None
    bangumi_id: Optional[str] = None
    assets_added: List[str] = []
    fields_updated: List[str] = []
    message: str


class BatchSyncRequest(BaseModel):
    """Request model for batch syncing."""
    force_steam: bool = Field(False, description="Force Steam enrichment")
    force_bangumi: bool = Field(False, description="Force Bangumi enrichment")
    download_assets: bool = Field(True, description="Download assets")
    limit: int = Field(100, description="Maximum games to enrich", ge=1, le=1000)


class StatusResponse(BaseModel):
    """Response model for connector status."""
    status: str
    connectors: Dict[str, Dict[str, Any]]
    library_root: str


class ConnectorStatus(BaseModel):
    """Status of a single connector."""
    name: str
    available: bool
    rate_limit_delay: float
    last_request: Optional[float] = None


# ============================================================================
# API Endpoints
# ============================================================================

@router.post("/sync", response_model=SyncResponse)
async def sync_game(request: SyncRequest):
    """
    Sync a game with external providers (Steam, Bangumi).

    Waterfall enrichment:
    1. Enrich Steam ID (if missing)
    2. Enrich Steam assets (if missing)
    3. Enrich Bangumi Chinese metadata (if not Chinese)

    Respects locked fields (Phase 10).

    Args:
        request: Sync request with game_path and options

    Returns:
        SyncResponse with enrichment results

    Example:
        POST /api/connectors/sync
        {
            "game_path": "Type-Moon/2004 Fate/stay night [v12345]",
            "force_steam": false,
            "force_bangumi": false,
            "download_assets": true
        }
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]

        # Build game folder path
        game_folder = library_root / request.game_path

        if not game_folder.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Game folder not found: {request.game_path}"
            )

        # Get enricher
        enricher = get_enricher(
            library_root=library_root,
            rate_limit_delay=1.0,
            download_assets=request.download_assets
        )

        # Perform enrichment
        result = enricher.enrich_game(
            game_folder=game_folder,
            force_steam=request.force_steam,
            force_bangumi=request.force_bangumi
        )

        return SyncResponse(
            success=result.success,
            game_path=request.game_path,
            steam_id=result.steam_id,
            bangumi_id=result.bangumi_id,
            assets_added=result.assets_added,
            fields_updated=result.fields_updated,
            message=result.message
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error syncing game: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error syncing game: {str(e)}"
        )


@router.post("/sync/batch")
async def batch_sync(request: BatchSyncRequest) -> Dict[str, Any]:
    """
    Sync multiple games with external providers.

    Processes all games in the library (up to limit).

    Args:
        request: Batch sync request with options

    Returns:
        Dict with enrichment statistics

    Example:
        POST /api/connectors/sync/batch
        {
            "force_steam": false,
            "force_bangumi": false,
            "download_assets": true,
            "limit": 100
        }
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]

        # Get enricher
        enricher = get_enricher(
            library_root=library_root,
            rate_limit_delay=1.0,
            download_assets=request.download_assets
        )

        # Scan for games
        game_folders = []
        for metadata_file in library_root.rglob("metadata.json"):
            if metadata_file.parent != library_root:
                game_folders.append(metadata_file.parent)
                if len(game_folders) >= request.limit:
                    break

        logger.info(f"Batch sync: processing {len(game_folders)} games")

        # Perform enrichment
        results = enricher.enrich_library(
            game_folders=game_folders,
            force_steam=request.force_steam,
            force_bangumi=request.force_bangumi
        )

        return {
            "success": True,
            "message": f"Processed {results['processed']} games",
            "results": results
        }

    except Exception as e:
        logger.error(f"Error in batch sync: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error in batch sync: {str(e)}"
        )


@router.post("/sync/all")
async def sync_all(
    force_steam: bool = Query(False, description="Force Steam enrichment"),
    force_bangumi: bool = Query(False, description="Force Bangumi enrichment")
) -> Dict[str, Any]:
    """
    Sync all games in the library.

    Alias for batch sync with no limit.

    Args:
        force_steam: Force Steam enrichment
        force_bangumi: Force Bangumi enrichment

    Returns:
        Dict with enrichment statistics

    Example:
        POST /api/connectors/sync/all?force_steam=false&force_bangumi=false
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]

        # Get enricher
        enricher = get_enricher(
            library_root=library_root,
            rate_limit_delay=1.0,
            download_assets=True
        )

        # Scan all games
        game_folders = []
        for metadata_file in library_root.rglob("metadata.json"):
            if metadata_file.parent != library_root:
                game_folders.append(metadata_file.parent)

        logger.info(f"Sync all: processing {len(game_folders)} games")

        # Perform enrichment
        results = enricher.enrich_library(
            game_folders=game_folders,
            force_steam=force_steam,
            force_bangumi=force_bangumi
        )

        return {
            "success": True,
            "message": f"Processed {results['processed']} games",
            "results": results
        }

    except Exception as e:
        logger.error(f"Error syncing all: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error syncing all: {str(e)}"
        )


@router.get("/status", response_model=StatusResponse)
async def get_connector_status() -> StatusResponse:
    """
    Get status of external connectors.

    Returns:
        StatusResponse with connector availability and info

    Example:
        GET /api/connectors/status
    """
    try:
        config = get_config()

        # Check connector availability
        connectors = {
            "steam": {
                "name": "Steam Store API",
                "available": True,  # Steam API is public
                "rate_limit_delay": 1.0,
                "requires_auth": False
            },
            "bangumi": {
                "name": "Bangumi API",
                "available": True,  # Bangumi API is public
                "rate_limit_delay": 1.0,
                "requires_auth": False
            }
        }

        return StatusResponse(
            status="operational",
            connectors=connectors,
            library_root=str(config.library_roots[0])
        )

    except Exception as e:
        logger.error(f"Error getting connector status: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting status: {str(e)}"
        )


@router.get("/test/{connector_name}")
async def test_connector(connector_name: str) -> Dict[str, Any]:
    """
    Test a specific connector connection.

    Args:
        connector_name: Name of connector (steam or bangumi)

    Returns:
        Dict with test results

    Example:
        GET /api/connectors/test/steam
    """
    try:
        if connector_name == "steam":
            from ..connectors.steam import get_steam_connector

            connector = get_steam_connector(rate_limit_delay=1.0)

            # Test search
            result = connector.search_by_title("Fate")

            return {
                "success": result is not None,
                "connector": "steam",
                "message": "Steam API is reachable",
                "test_result": {
                    "found": result is not None,
                    "sample_data": result if result else None
                }
            }

        elif connector_name == "bangumi":
            from ..connectors.bangumi import get_bangumi_connector

            connector = get_bangumi_connector(rate_limit_delay=1.0)

            # Test search
            results = connector.search_by_title("Fate", max_results=1)

            return {
                "success": len(results) > 0,
                "connector": "bangumi",
                "message": "Bangumi API is reachable",
                "test_result": {
                    "found_count": len(results),
                    "sample_data": results[0] if results else None
                }
            }

        else:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"Unknown connector: {connector_name}"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error testing connector {connector_name}: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error testing connector: {str(e)}"
        )
