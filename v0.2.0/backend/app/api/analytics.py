"""
Analytics API endpoints for Galgame Library Manager.

**PHASE 11: The Explorer Backend**

Provides REST API endpoints for:
- Dashboard statistics
- Knowledge graph (staff, cast, series)
"""

import logging
from typing import Dict, Any

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel, Field

from ..analytics.stats import get_stats_engine
from ..analytics.graph import get_graph_engine

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/analytics", tags=["analytics"])


# ============================================================================
# Pydantic Models
# ============================================================================

class DashboardStatsResponse(BaseModel):
    """Response model for dashboard statistics."""
    total_games: int
    timeline: Dict[str, int] = Field(..., description="Games by release year")
    engines: Dict[str, int] = Field(..., description="Games by engine")
    play_time: Dict[str, int] = Field(..., description="Games by length")
    tags: list = Field(..., description="Top 50 tags with weights")


class StaffSearchResponse(BaseModel):
    """Response model for staff search."""
    name: str
    matched_names: list
    total_games: int
    games_by_role: Dict[str, list]


class CastSearchResponse(BaseModel):
    """Response model for cast search."""
    name: str
    matched_names: list
    total_characters: int
    games: list


class SeriesSearchResponse(BaseModel):
    """Response model for series search."""
    name: str
    matched_aliases: list
    total_games: int
    games: list


# ============================================================================
# API Endpoints
# ============================================================================

@router.get("/dashboard", response_model=DashboardStatsResponse)
async def get_dashboard_stats():
    """
    Get dashboard statistics for the Focus panel.

    Returns aggregated data across the entire library:
    - Timeline: Distribution by release year
    - Engines: Count by game engine
    - Play Time: Distribution by length
    - Tags: Weighted tag cloud (top 50)

    Returns:
        DashboardStatsResponse with all stats
    """
    try:
        stats_engine = get_stats_engine()
        stats = stats_engine.get_dashboard_stats()

        return DashboardStatsResponse(**stats)

    except Exception as e:
        logger.error(f"Error getting dashboard stats: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting dashboard stats: {str(e)}"
        )


@router.get("/staff/{name}", response_model=StaffSearchResponse)
async def search_staff(name: str):
    """
    Search for staff member across all games.

    Returns games grouped by role (scenario, art, music, etc.).

    Args:
        name: Staff name to search (case-insensitive, partial match)

    Returns:
        StaffSearchResponse with games grouped by role
    """
    try:
        graph_engine = get_graph_engine()
        result = graph_engine.search_staff(name)

        return StaffSearchResponse(**result)

    except Exception as e:
        logger.error(f"Error searching staff: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error searching staff: {str(e)}"
        )


@router.get("/cast/{name}", response_model=CastSearchResponse)
async def search_cast(name: str):
    """
    Search for voice actor across all games.

    Returns games with characters voiced.

    Args:
        name: Voice actor name to search (case-insensitive, partial match)

    Returns:
        CastSearchResponse with games and characters
    """
    try:
        graph_engine = get_graph_engine()
        result = graph_engine.search_cast(name)

        return CastSearchResponse(**result)

    except Exception as e:
        logger.error(f"Error searching cast: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error searching cast: {str(e)}"
        )


@router.get("/series/{series_name}", response_model=SeriesSearchResponse)
async def search_series(series_name: str):
    """
    Search for games in a series/franchise.

    Uses fuzzy matching on title and aliases.

    Args:
        series_name: Series name to search (case-insensitive, partial match)

    Returns:
        SeriesSearchResponse with games sorted by year
    """
    try:
        graph_engine = get_graph_engine()
        result = graph_engine.search_series(series_name)

        return SeriesSearchResponse(**result)

    except Exception as e:
        logger.error(f"Error searching series: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error searching series: {str(e)}"
        )
