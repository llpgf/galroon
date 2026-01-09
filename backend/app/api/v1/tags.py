"""
Tags API for Galroon v0.2.0

Sprint 9: Tag Management System

Endpoints:
- GET  /api/v1/tags              - List all tags with game counts
- POST /api/v1/tags              - Create new tag
- PATCH /api/v1/tags/{id}        - Rename tag
- DELETE /api/v1/tags/{id}       - Delete tag
- POST /api/v1/tags/{id}/apply   - Batch apply to games
- POST /api/v1/tags/{id}/remove  - Batch remove from games
"""

import logging
import uuid
from typing import List, Optional
from datetime import datetime

from fastapi import APIRouter, HTTPException, Depends
from pydantic import BaseModel, Field

from ...core.database import get_database

logger = logging.getLogger(__name__)

router = APIRouter(tags=["tags"])


# ============================================================================
# DTOs
# ============================================================================

class TagCreate(BaseModel):
    """Request to create a new tag."""
    name: str = Field(..., min_length=1, max_length=50, description="Tag name")
    color: str = Field(default="#8B5CF6", description="Tag color (hex)")


class TagUpdate(BaseModel):
    """Request to update a tag."""
    name: Optional[str] = Field(None, min_length=1, max_length=50)
    color: Optional[str] = None


class TagResponse(BaseModel):
    """Tag with metadata."""
    id: str
    name: str
    color: str
    game_count: int = 0
    created_at: Optional[str] = None


class TagListResponse(BaseModel):
    """List of tags."""
    tags: List[TagResponse]
    total: int


class BatchApplyRequest(BaseModel):
    """Request to batch apply/remove tag from games."""
    game_ids: List[str] = Field(..., description="List of game IDs")


class BatchApplyResponse(BaseModel):
    """Response for batch operations."""
    success: bool
    affected_count: int
    message: str


# ============================================================================
# CRUD Endpoints
# ============================================================================

@router.get("/tags", response_model=TagListResponse)
async def list_tags(db=Depends(get_database)):
    """
    List all tags with game counts.
    """
    try:
        with db.get_cursor() as cursor:
            # Get all tags with game counts
            cursor.execute("""
                SELECT 
                    t.id,
                    t.name,
                    t.color,
                    t.created_at,
                    COUNT(gt.game_id) as game_count
                FROM tags t
                LEFT JOIN game_tags gt ON t.id = gt.tag_id
                GROUP BY t.id
                ORDER BY t.name COLLATE NOCASE
            """)
            
            rows = cursor.fetchall()
            tags = []
            for row in rows:
                row_dict = dict(row)
                tags.append(TagResponse(
                    id=row_dict['id'],
                    name=row_dict['name'],
                    color=row_dict['color'] or '#8B5CF6',
                    game_count=row_dict['game_count'] or 0,
                    created_at=row_dict.get('created_at')
                ))
            
            return TagListResponse(tags=tags, total=len(tags))
    
    except Exception as e:
        logger.error(f"Failed to list tags: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/tags", response_model=TagResponse)
async def create_tag(request: TagCreate, db=Depends(get_database)):
    """
    Create a new tag.
    """
    try:
        tag_id = str(uuid.uuid4())
        created_at = datetime.utcnow().isoformat()
        
        with db.get_cursor() as cursor:
            # Check if tag name already exists
            cursor.execute("SELECT id FROM tags WHERE name = ?", (request.name,))
            existing = cursor.fetchone()
            if existing:
                raise HTTPException(status_code=400, detail=f"Tag '{request.name}' already exists")
            
            # Insert new tag
            cursor.execute("""
                INSERT INTO tags (id, name, color, created_at)
                VALUES (?, ?, ?, ?)
            """, (tag_id, request.name, request.color, created_at))
            
            db.conn.commit()
            
            logger.info(f"Created tag: {request.name} ({tag_id})")
            
            return TagResponse(
                id=tag_id,
                name=request.name,
                color=request.color,
                game_count=0,
                created_at=created_at
            )
    
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Failed to create tag: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.patch("/tags/{tag_id}", response_model=TagResponse)
async def update_tag(tag_id: str, request: TagUpdate, db=Depends(get_database)):
    """
    Update tag name or color.
    """
    try:
        with db.get_cursor() as cursor:
            # Check if tag exists
            cursor.execute("SELECT * FROM tags WHERE id = ?", (tag_id,))
            existing = cursor.fetchone()
            if not existing:
                raise HTTPException(status_code=404, detail=f"Tag not found: {tag_id}")
            
            existing_dict = dict(existing)
            
            # Check if new name conflicts
            if request.name and request.name != existing_dict['name']:
                cursor.execute("SELECT id FROM tags WHERE name = ? AND id != ?", (request.name, tag_id))
                conflict = cursor.fetchone()
                if conflict:
                    raise HTTPException(status_code=400, detail=f"Tag '{request.name}' already exists")
            
            # Update fields
            new_name = request.name or existing_dict['name']
            new_color = request.color or existing_dict['color']
            
            cursor.execute("""
                UPDATE tags SET name = ?, color = ? WHERE id = ?
            """, (new_name, new_color, tag_id))
            
            db.conn.commit()
            
            # Get game count
            cursor.execute("SELECT COUNT(*) FROM game_tags WHERE tag_id = ?", (tag_id,))
            game_count = cursor.fetchone()[0]
            
            logger.info(f"Updated tag: {new_name} ({tag_id})")
            
            return TagResponse(
                id=tag_id,
                name=new_name,
                color=new_color,
                game_count=game_count,
                created_at=existing_dict.get('created_at')
            )
    
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Failed to update tag: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.delete("/tags/{tag_id}")
async def delete_tag(tag_id: str, db=Depends(get_database)):
    """
    Delete a tag and remove all associations.
    """
    try:
        with db.get_cursor() as cursor:
            # Check if tag exists
            cursor.execute("SELECT name FROM tags WHERE id = ?", (tag_id,))
            existing = cursor.fetchone()
            if not existing:
                raise HTTPException(status_code=404, detail=f"Tag not found: {tag_id}")
            
            tag_name = existing[0]
            
            # Delete associations first
            cursor.execute("DELETE FROM game_tags WHERE tag_id = ?", (tag_id,))
            
            # Delete tag
            cursor.execute("DELETE FROM tags WHERE id = ?", (tag_id,))
            
            db.conn.commit()
            
            logger.info(f"Deleted tag: {tag_name} ({tag_id})")
            
            return {"success": True, "message": f"Tag '{tag_name}' deleted"}
    
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Failed to delete tag: {e}")
        raise HTTPException(status_code=500, detail=str(e))


# ============================================================================
# Batch Operations
# ============================================================================

@router.post("/tags/{tag_id}/apply", response_model=BatchApplyResponse)
async def batch_apply_tag(tag_id: str, request: BatchApplyRequest, db=Depends(get_database)):
    """
    Apply a tag to multiple games.
    """
    try:
        with db.get_cursor() as cursor:
            # Verify tag exists
            cursor.execute("SELECT name FROM tags WHERE id = ?", (tag_id,))
            tag = cursor.fetchone()
            if not tag:
                raise HTTPException(status_code=404, detail=f"Tag not found: {tag_id}")
            
            tag_name = tag[0]
            applied_count = 0
            
            for game_id in request.game_ids:
                # Check if already applied
                cursor.execute(
                    "SELECT 1 FROM game_tags WHERE game_id = ? AND tag_id = ?",
                    (game_id, tag_id)
                )
                if not cursor.fetchone():
                    cursor.execute(
                        "INSERT INTO game_tags (game_id, tag_id) VALUES (?, ?)",
                        (game_id, tag_id)
                    )
                    applied_count += 1
            
            db.conn.commit()
            
            logger.info(f"Applied tag '{tag_name}' to {applied_count} games")
            
            return BatchApplyResponse(
                success=True,
                affected_count=applied_count,
                message=f"Applied '{tag_name}' to {applied_count} games"
            )
    
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Failed to batch apply tag: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/tags/{tag_id}/remove", response_model=BatchApplyResponse)
async def batch_remove_tag(tag_id: str, request: BatchApplyRequest, db=Depends(get_database)):
    """
    Remove a tag from multiple games.
    """
    try:
        with db.get_cursor() as cursor:
            # Verify tag exists
            cursor.execute("SELECT name FROM tags WHERE id = ?", (tag_id,))
            tag = cursor.fetchone()
            if not tag:
                raise HTTPException(status_code=404, detail=f"Tag not found: {tag_id}")
            
            tag_name = tag[0]
            
            # Remove associations
            placeholders = ','.join(['?'] * len(request.game_ids))
            cursor.execute(f"""
                DELETE FROM game_tags 
                WHERE tag_id = ? AND game_id IN ({placeholders})
            """, [tag_id] + request.game_ids)
            
            removed_count = cursor.rowcount
            db.conn.commit()
            
            logger.info(f"Removed tag '{tag_name}' from {removed_count} games")
            
            return BatchApplyResponse(
                success=True,
                affected_count=removed_count,
                message=f"Removed '{tag_name}' from {removed_count} games"
            )
    
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Failed to batch remove tag: {e}")
        raise HTTPException(status_code=500, detail=str(e))
