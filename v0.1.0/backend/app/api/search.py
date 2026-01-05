"""
Advanced Focus Search API for Galgame Library Manager.

**PHASE 11: Advanced Search**

Provides faceted search with combined filters:
- Year range (year_min, year_max)
- Tags
- Engine
- Developer
- Text search
"""

import logging
from pathlib import Path
from typing import Dict, Any, List, Optional

from fastapi import APIRouter, HTTPException, status, Query
from pydantic import BaseModel, Field

from ..config import get_config
from ..metadata.manager import get_resource_manager
from ..metadata.models import UnifiedMetadata

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/search", tags=["search"])


# ============================================================================
# Pydantic Models
# ============================================================================

class SearchResult(BaseModel):
    """Single search result."""
    vndb_id: Optional[str] = None
    title: str
    folder_path: str
    year: Optional[str] = None
    developer: Optional[str] = None
    engine: Optional[str] = None
    tags: List[str] = []
    description: Optional[str] = None
    cover_url: Optional[str] = None


class SearchResponse(BaseModel):
    """Response model for search results."""
    total: int
    results: List[SearchResult]
    filters_applied: Dict[str, Any]


# ============================================================================
# Search Engine
# ============================================================================

class AdvancedSearchEngine:
    """
    Advanced search with faceted filters.

    Supports:
    - Text search (title, description)
    - Year range filtering
    - Tag filtering
    - Engine filtering
    - Developer filtering
    """

    def __init__(self):
        """Initialize search engine."""
        self.config = get_config()
        self.library_roots = self.config.library_roots

    def load_all_metadata(self) -> List[UnifiedMetadata]:
        """
        Load all metadata from library roots.

        Returns:
            List of all metadata objects
        """
        all_metadata = []

        for library_root in self.library_roots:
            if not library_root.exists():
                logger.warning(f"Library root does not exist: {library_root}")
                continue

            resource_manager = get_resource_manager(library_root, quota_gb=2.0)

            # Find all metadata.json files
            for metadata_file in library_root.rglob("metadata.json"):
                try:
                    metadata_dict = resource_manager.load_metadata(metadata_file.parent)
                    if metadata_dict:
                        metadata = UnifiedMetadata(**metadata_dict)
                        all_metadata.append(metadata)
                except Exception as e:
                    logger.warning(f"Failed to load metadata from {metadata_file}: {e}")

        logger.info(f"Loaded {len(all_metadata)} metadata objects")
        return all_metadata

    def search(
        self,
        query: Optional[str] = None,
        year_min: Optional[int] = None,
        year_max: Optional[int] = None,
        tags: Optional[List[str]] = None,
        engine: Optional[str] = None,
        developer: Optional[str] = None,
        limit: int = 100
    ) -> tuple[List[Dict[str, Any]], Dict[str, Any]]:
        """
        Perform advanced faceted search.

        Args:
            query: Text search query (searches title and description)
            year_min: Minimum release year (inclusive)
            year_max: Maximum release year (inclusive)
            tags: List of required tags (AND logic)
            engine: Filter by engine name
            developer: Filter by developer name
            limit: Maximum results to return

        Returns:
            Tuple of (results_list, filters_applied_dict)
        """
        # Load all metadata
        metadata_list = self.load_all_metadata()

        # Track filters applied
        filters_applied = {
            "query": query,
            "year_min": year_min,
            "year_max": year_max,
            "tags": tags,
            "engine": engine,
            "developer": developer
        }

        results = []
        query_lower = query.lower() if query else None

        for metadata in metadata_list:
            # Extract fields
            title = metadata.title.value.original if metadata.title else ""
            description = metadata.description.value if metadata.description else ""
            release_date = metadata.release_date.value if metadata.release_date else None
            game_tags = metadata.tags.value if metadata.tags else []
            game_engine = metadata.engine.value if metadata.engine else None
            game_developer = metadata.developer.value if metadata.developer else None
            cover_url = metadata.cover_url.value if metadata.cover_url else None

            # Extract year from release date
            year = None
            if release_date:
                try:
                    year = int(release_date[:4])
                except (ValueError, TypeError):
                    pass

            # Apply filters

            # Text search
            if query_lower:
                title_match = query_lower in title.lower()
                desc_match = query_lower in description.lower() if description else False
                if not (title_match or desc_match):
                    continue

            # Year range
            if year_min is not None and (year is None or year < year_min):
                continue
            if year_max is not None and (year is None or year > year_max):
                continue

            # Tags (AND logic - all tags must be present)
            if tags and isinstance(tags, list) and len(tags) > 0:
                if not isinstance(game_tags, list):
                    continue
                # Check if all required tags are present
                game_tags_lower = [t.lower() for t in game_tags if isinstance(t, str)]
                if not all(tag.lower() in game_tags_lower for tag in tags):
                    continue

            # Engine
            if engine and game_engine != engine:
                continue

            # Developer
            if developer:
                if not game_developer or developer.lower() not in game_developer.lower():
                    continue

            # All filters passed, add to results
            results.append({
                "vndb_id": metadata.external_ids.vndb if metadata.external_ids else None,
                "title": title,
                "folder_path": str(metadata.folder_path) if hasattr(metadata, 'folder_path') else "",
                "year": str(year) if year else None,
                "developer": game_developer,
                "engine": game_engine,
                "tags": game_tags if isinstance(game_tags, list) else [],
                "description": description[:500] if description else None,  # Truncate for preview
                "cover_url": cover_url
            })

        # Sort by relevance (title match first, then year)
        if query_lower:
            results.sort(key=lambda x: (
                0 if query_lower in x["title"].lower() else 1,  # Title matches first
                -(int(x["year"]) if x["year"] else 0)  # Then newest first
            ))
        else:
            # No text query, sort by year descending
            results.sort(key=lambda x: -(int(x["year"]) if x["year"] else 0))

        # Apply limit
        results = results[:limit]

        return results, filters_applied


# Singleton instance
_search_engine: Optional[AdvancedSearchEngine] = None


def get_search_engine() -> AdvancedSearchEngine:
    """
    Get or create search engine singleton.

    Returns:
        AdvancedSearchEngine instance
    """
    global _search_engine
    if _search_engine is None:
        _search_engine = AdvancedSearchEngine()
    return _search_engine


# ============================================================================
# API Endpoints
# ============================================================================

@router.get("/", response_model=SearchResponse)
async def advanced_search(
    query: Optional[str] = Query(None, description="Text search query"),
    year_min: Optional[int] = Query(None, description="Minimum release year", ge=1900, le=2100),
    year_max: Optional[int] = Query(None, description="Maximum release year", ge=1900, le=2100),
    tags: Optional[str] = Query(None, description="Comma-separated list of tags (AND logic)"),
    engine: Optional[str] = Query(None, description="Filter by engine"),
    developer: Optional[str] = Query(None, description="Filter by developer"),
    limit: int = Query(100, description="Maximum results", ge=1, le=500)
):
    """
    Advanced faceted search across the library.

    Supports combining multiple filters:
    - Text search (searches title and description)
    - Year range (year_min, year_max)
    - Tags (comma-separated, AND logic)
    - Engine name
    - Developer name

    Returns:
        SearchResponse with matching games
    """
    try:
        search_engine = get_search_engine()

        # Parse tags comma-separated string
        tags_list = None
        if tags:
            tags_list = [t.strip() for t in tags.split(",") if t.strip()]

        # Perform search
        results, filters_applied = search_engine.search(
            query=query,
            year_min=year_min,
            year_max=year_max,
            tags=tags_list,
            engine=engine,
            developer=developer,
            limit=limit
        )

        return SearchResponse(
            total=len(results),
            results=results,
            filters_applied=filters_applied
        )

    except Exception as e:
        logger.error(f"Error performing search: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error performing search: {str(e)}"
        )


@router.get("/facets")
async def get_available_facets():
    """
    Get available facet values for advanced search.

    Returns:
        Dict with available tags, engines, developers, and year range
    """
    try:
        search_engine = get_search_engine()
        metadata_list = search_engine.load_all_metadata()

        # Aggregate unique values
        tags_set = set()
        engines_set = set()
        developers_set = set()
        years = []

        for metadata in metadata_list:
            # Tags
            if metadata.tags and metadata.tags.value:
                if isinstance(metadata.tags.value, list):
                    for tag in metadata.tags.value:
                        if isinstance(tag, str):
                            tags_set.add(tag)

            # Engine
            if metadata.engine and metadata.engine.value:
                engines_set.add(metadata.engine.value)

            # Developer
            if metadata.developer and metadata.developer.value:
                developers_set.add(metadata.developer.value)

            # Year
            if metadata.release_date and metadata.release_date.value:
                try:
                    year = int(metadata.release_date.value[:4])
                    years.append(year)
                except (ValueError, TypeError):
                    pass

        # Sort and convert to lists
        result = {
            "tags": sorted(list(tags_set)),
            "engines": sorted(list(engines_set)),
            "developers": sorted(list(developers_set)),
            "year_range": {
                "min": min(years) if years else None,
                "max": max(years) if years else None
            }
        }

        return result

    except Exception as e:
        logger.error(f"Error getting facets: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting facets: {str(e)}"
        )
