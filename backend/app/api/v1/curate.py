"""
Curate API - Sprint 9.5: Gallery/Workshop Dual View

This module provides endpoints for managing the curation state of canonical games.
Games with is_curated=True are shown in the Gallery (exhibition hall).
Games with is_curated=False remain in the Workshop (pending area).
"""

from fastapi import APIRouter, HTTPException, Depends
from pydantic import BaseModel
from typing import List, Optional
import logging

from app.core.database import Database

logger = logging.getLogger(__name__)
router = APIRouter(prefix="/canonical", tags=["curate"])


class CurateRequest(BaseModel):
    """Request body for single curate action."""
    curate: bool = True


class BatchCurateRequest(BaseModel):
    """Request body for batch curate action."""
    ids: List[str]
    curate: bool = True


class CurateResponse(BaseModel):
    """Response for curate actions."""
    success: bool
    message: str
    updated_count: int = 1
    data: Optional[dict] = None  # Added for returning updated game data


class UpdateGameRequest(BaseModel):
    """Request body for updating canonical game metadata."""
    display_title: Optional[str] = None
    description: Optional[str] = None
    developer: Optional[str] = None
    release_date: Optional[str] = None
    cover_image_url: Optional[str] = None
    tags: Optional[List[str]] = None


def get_database() -> Database:
    """Get database instance."""
    return Database()


@router.patch("/{canonical_id}", response_model=CurateResponse)
async def update_canonical_game(
    canonical_id: str,
    request: UpdateGameRequest,
    db: Database = Depends(get_database)
):
    """
    Update canonical game metadata (The Truth).
    
    This endpoint allows the Metadata Editor to directly modify
    the canonical source of truth for a game.
    """
    try:
        # Build update query dynamically
        update_fields = []
        params = []
        
        if request.display_title is not None:
            update_fields.append("display_title = ?")
            params.append(request.display_title)
            
        if request.cover_image_url is not None:
            update_fields.append("cover_image_url = ?")
            params.append(request.cover_image_url)

        # For metadata fields stored in JSON blob, we need to merge
        # This is a simplification: for now we only update top-level columns
        # Ideally we would update the metadata_snapshot JSON too
        
        if not update_fields:
             raise HTTPException(status_code=400, detail="No fields to update")

        update_fields.append("updated_at = CURRENT_TIMESTAMP")
        
        # Add ID for WHERE clause
        params.append(canonical_id)
        
        with db.get_connection() as conn:
            cursor = conn.cursor()
            
            # Check existence
            cursor.execute("SELECT id, metadata_snapshot FROM canonical_games WHERE id = ?", (canonical_id,))
            game = cursor.fetchone()
            if not game:
                raise HTTPException(status_code=404, detail="Canonical game not found")

            # Handle JSON Metadata updates (description, developer, release_date, tags)
            metadata = {}
            if game['metadata_snapshot']:
                try:
                    import json
                    metadata = json.loads(game['metadata_snapshot'])
                except:
                    metadata = {}
            
            metadata_changed = False
            if request.description is not None:
                metadata['description'] = request.description
                metadata_changed = True
            if request.developer is not None:
                metadata['developer'] = request.developer
                metadata_changed = True
            if request.release_date is not None:
                metadata['release_date'] = request.release_date
                metadata_changed = True
            if request.tags is not None:
                metadata['tags'] = request.tags
                metadata_changed = True
            
            if metadata_changed:
                import json
                update_fields.insert(0, "metadata_snapshot = ?")
                params.insert(0, json.dumps(metadata))

            query = f"""
                UPDATE canonical_games 
                SET {', '.join(update_fields)}
                WHERE id = ?
            """
            
            cursor.execute(query, params)
            conn.commit()
            
            logger.info(f"Updated canonical game {canonical_id}")
            
            return CurateResponse(
                success=True,
                message="Game metadata updated successfully",
                data={
                    "id": canonical_id,
                    **request.dict(exclude_unset=True)
                }
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Failed to update game {canonical_id}: {e}")
        raise HTTPException(status_code=500, detail=str(e))



@router.post("/{canonical_id}/curate", response_model=CurateResponse)
async def curate_game(
    canonical_id: str,
    request: CurateRequest,
    db: Database = Depends(get_database)
):
    """
    Curate a single canonical game.
    
    This is the "ritual" endpoint that moves a game between Workshop and Gallery.
    - curate=True: Move to Gallery (is_curated=1)
    - curate=False: Move back to Workshop (is_curated=0)
    """
    try:
        # Check if game exists
        with db.get_connection() as conn:
            cursor = conn.cursor()
            cursor.execute(
                "SELECT id, display_title FROM canonical_games WHERE id = ?",
                (canonical_id,)
            )
            game = cursor.fetchone()
            
            if not game:
                raise HTTPException(
                    status_code=404,
                    detail=f"Canonical game not found: {canonical_id}"
                )
            
            # Update curation status
            cursor.execute(
                """
                UPDATE canonical_games 
                SET is_curated = ?, updated_at = CURRENT_TIMESTAMP
                WHERE id = ?
                """,
                (1 if request.curate else 0, canonical_id)
            )
            conn.commit()
            
            action = "curated" if request.curate else "uncurated"
            logger.info(f"Game {game['display_title']} ({canonical_id}) {action}")
            
            return CurateResponse(
                success=True,
                message=f"Game '{game['display_title']}' has been {action}",
                updated_count=1
            )
            
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Failed to curate game {canonical_id}: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/batch-curate", response_model=CurateResponse)
async def batch_curate_games(
    request: BatchCurateRequest,
    db: Database = Depends(get_database)
):
    """
    Batch curate multiple canonical games.
    
    Efficiently updates curation status for multiple games at once.
    This is used when the user selects multiple items in the Workshop
    and clicks "Batch Curate".
    """
    if not request.ids:
        raise HTTPException(
            status_code=400,
            detail="No game IDs provided"
        )
    
    try:
        with db.get_connection() as conn:
            cursor = conn.cursor()
            
            # Build placeholders for IN clause
            placeholders = ",".join(["?" for _ in request.ids])
            
            # Update all matching games
            cursor.execute(
                f"""
                UPDATE canonical_games 
                SET is_curated = ?, updated_at = CURRENT_TIMESTAMP
                WHERE id IN ({placeholders})
                """,
                [1 if request.curate else 0] + request.ids
            )
            
            updated_count = cursor.rowcount
            conn.commit()
            
            action = "curated" if request.curate else "uncurated"
            logger.info(f"Batch {action} {updated_count} games")
            
            return CurateResponse(
                success=True,
                message=f"{updated_count} games have been {action}",
                updated_count=updated_count
            )
            
    except Exception as e:
        logger.error(f"Failed to batch curate games: {e}")
        raise HTTPException(status_code=500, detail=str(e))
