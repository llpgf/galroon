"""
Metadata API v1 - Sprint 10: Per-Field Authority Tracking

Implements the dual-track metadata editing system where:
- API suggestions can be selectively adopted
- User edits take precedence as "manual" authority
- Each field tracks its source independently

Endpoints:
- PATCH /canonical/{id}: Per-field metadata update with authority tracking
- GET /vndb/{vndb_id}/images: Fetch VNDB images for cover picker
"""

import json
import logging
from typing import Dict, Any, Optional, List
from datetime import datetime

from fastapi import APIRouter, HTTPException, status, Depends
from pydantic import BaseModel, Field, ConfigDict

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/canonical", tags=["metadata"])


# ============================================================================
# Pydantic Models
# ============================================================================

class MetadataPatchRequest(BaseModel):
    """
    Request model for per-field metadata update.
    
    Only fields present in the request will be updated.
    Each updated field is marked as "manual" in overridden_fields.
    """
    display_title: Optional[str] = Field(None, description="Display title")
    description: Optional[str] = Field(None, description="Game description")
    developer: Optional[str] = Field(None, description="Developer name")
    release_date: Optional[str] = Field(None, description="Release date (YYYY-MM-DD)")
    cover_image_url: Optional[str] = Field(None, description="Cover image URL")
    tags: Optional[List[str]] = Field(None, description="List of tags")
    # Source override: if set, marks field as coming from API instead of manual
    source_overrides: Optional[Dict[str, str]] = Field(
        None, 
        description="Explicitly set source for fields: {'field': 'api'|'manual'}"
    )

    model_config = ConfigDict(extra='allow')


class MetadataPatchResponse(BaseModel):
    """Response model for metadata patch."""
    success: bool
    canonical_id: str
    updated_fields: List[str]
    overridden_fields: Dict[str, str]
    message: str


class VNDBImagesResponse(BaseModel):
    """Response model for VNDB images endpoint."""
    vndb_id: str
    cover: Optional[str] = None
    screenshots: List[str] = []


# ============================================================================
# API Endpoints
# ============================================================================

@router.patch("/{canonical_id}", response_model=MetadataPatchResponse)
async def patch_canonical_metadata(canonical_id: str, request: MetadataPatchRequest):
    """
    Update canonical game metadata with per-field authority tracking.
    
    Rules:
    - Only fields present in the request are updated
    - Fields NOT in payload remain unchanged
    - Updated fields are marked as "manual" in overridden_fields
    - Use source_overrides to explicitly set source (e.g., re-link to API)
    
    Args:
        canonical_id: Canonical game ID
        request: MetadataPatchRequest with fields to update
    
    Returns:
        MetadataPatchResponse with list of updated fields
    
    Example:
        PATCH /api/v1/canonical/abc123
        {"display_title": "New Title", "developer": "Studio Name"}
        
        Result: overridden_fields = {"display_title": "manual", "developer": "manual"}
    """
    from ...core.database import Database
    
    try:
        db = Database()
        
        # Fetch existing canonical game
        row = db.conn.execute(
            "SELECT * FROM canonical_games WHERE id = ?",
            (canonical_id,)
        ).fetchone()
        
        if not row:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Canonical game not found: {canonical_id}"
            )
        
        # Parse existing data
        existing_data = dict(row)
        metadata_snapshot = json.loads(existing_data.get('metadata_snapshot', '{}'))
        overridden_fields = json.loads(existing_data.get('overridden_fields', '{}'))
        
        # Track which fields we're updating
        updated_fields = []
        request_dict = request.dict(exclude_none=True, exclude={'source_overrides'})
        
        # Process field updates
        for field, value in request_dict.items():
            if field == 'display_title':
                existing_data['display_title'] = value
                overridden_fields['display_title'] = 'manual'
                updated_fields.append('display_title')
            elif field == 'cover_image_url':
                existing_data['cover_image_url'] = value
                overridden_fields['cover_image_url'] = 'manual'
                updated_fields.append('cover_image_url')
            elif field in ['description', 'developer', 'release_date', 'tags']:
                # These go into metadata_snapshot
                metadata_snapshot[field] = value
                overridden_fields[field] = 'manual'
                updated_fields.append(field)
            else:
                # Other fields go into metadata_snapshot as well
                metadata_snapshot[field] = value
                overridden_fields[field] = 'manual'
                updated_fields.append(field)
        
        # Process source overrides (e.g., re-link to API)
        if request.source_overrides:
            for field, source in request.source_overrides.items():
                if source in ['api', 'manual']:
                    overridden_fields[field] = source
                    if field not in updated_fields:
                        updated_fields.append(field)
        
        # Update database
        db.conn.execute(
            """
            UPDATE canonical_games 
            SET display_title = ?,
                cover_image_url = ?,
                metadata_snapshot = ?,
                overridden_fields = ?,
                updated_at = ?
            WHERE id = ?
            """,
            (
                existing_data['display_title'],
                existing_data.get('cover_image_url'),
                json.dumps(metadata_snapshot),
                json.dumps(overridden_fields),
                datetime.now().isoformat(),
                canonical_id
            )
        )
        db.conn.commit()
        
        logger.info(f"Updated metadata for {canonical_id}: {updated_fields}")
        
        return MetadataPatchResponse(
            success=True,
            canonical_id=canonical_id,
            updated_fields=updated_fields,
            overridden_fields=overridden_fields,
            message=f"Updated {len(updated_fields)} field(s)"
        )
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error updating metadata for {canonical_id}: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to update metadata: {str(e)}"
        )


# ============================================================================
# VNDB Images Endpoint
# ============================================================================

vndb_router = APIRouter(prefix="/vndb", tags=["vndb"])


@vndb_router.get("/{vndb_id}/images", response_model=VNDBImagesResponse)
async def get_vndb_images(vndb_id: str):
    """
    Fetch images from VNDB for the cover gallery picker.
    
    Returns cover image and screenshot URLs without any auto-selection.
    Frontend displays all options for user to choose.
    
    Args:
        vndb_id: VNDB ID (e.g., "v12345")
    
    Returns:
        VNDBImagesResponse with cover and screenshots array
    """
    from ...metadata.providers.vndb import VNDBProvider
    
    try:
        provider = VNDBProvider()
        
        # Normalize ID format
        if not vndb_id.startswith('v'):
            vndb_id = f"v{vndb_id}"
        
        # Fetch metadata which includes images
        data = provider.get_metadata_by_id(vndb_id)
        
        if not data:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"VNDB entry not found: {vndb_id}"
            )
        
        # Extract images
        cover = data.get('image', {}).get('url') if isinstance(data.get('image'), dict) else data.get('image')
        
        screenshots = []
        if 'screenshots' in data and data['screenshots']:
            for screenshot in data['screenshots']:
                if isinstance(screenshot, dict) and 'url' in screenshot:
                    screenshots.append(screenshot['url'])
                elif isinstance(screenshot, str):
                    screenshots.append(screenshot)
        
        return VNDBImagesResponse(
            vndb_id=vndb_id,
            cover=cover,
            screenshots=screenshots
        )
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error fetching VNDB images for {vndb_id}: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to fetch VNDB images: {str(e)}"
        )
