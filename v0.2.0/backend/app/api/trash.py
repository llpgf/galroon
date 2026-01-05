"""
Trash API endpoints for Galgame Library Manager.

**PHASE 19.5: Trash Management with Journal Logging**

Provides REST API endpoints for managing:
- Trash status
- Trash configuration
- Empty trash with journal logging
"""

import logging
import json
from pathlib import Path
from typing import Dict, Any

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel, Field

from ..config import get_config
from ..core.journal import JournalManager

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/trash", tags=["trash"])


# ============================================================================
# Pydantic Models
# ============================================================================

class TrashStatusResponse(BaseModel):
    """Response model for trash status."""
    trash_items: int = Field(..., description="Number of items in trash")
    trash_size_gb: float = Field(..., description="Total size of trash in GB")


class TrashConfigResponse(BaseModel):
    """Response model for trash configuration."""
    max_size_gb: float = Field(..., description="Maximum trash size in GB")
    retention_days: int = Field(..., description="Retention period in days")
    min_disk_free_gb: float = Field(..., description="Minimum free disk space in GB")


class EmptyTrashResponse(BaseModel):
    """Response model for empty trash operation."""
    success: bool
    message: str
    items_deleted: int


# ============================================================================
# API Endpoints
# ============================================================================

@router.get("/status", response_model=TrashStatusResponse)
async def get_trash_status():
    """
    Get current trash status.

    Returns:
        TrashStatusResponse with current trash state

    Example:
        GET /api/trash/status
    """
    try:
        config = get_config()
        trash_dir = config.trash_dir

        if not trash_dir.exists():
            return TrashStatusResponse(trash_items=0, trash_size_gb=0.0)

        # Count items and calculate size
        trash_items = 0
        trash_size_bytes = 0

        for item in trash_dir.rglob("*"):
            if item.is_file():
                trash_items += 1
                trash_size_bytes += item.stat().st_size

        trash_size_gb = trash_size_bytes / (1024 ** 3)

        return TrashStatusResponse(
            trash_items=trash_items,
            trash_size_gb=round(trash_size_gb, 2)
        )

    except Exception as e:
        logger.error(f"Error getting trash status: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting trash status: {str(e)}"
        )


@router.get("/config", response_model=TrashConfigResponse)
async def get_trash_config():
    """
    Get trash configuration.

    Returns:
        TrashConfigResponse with current settings

    Example:
        GET /api/trash/config
    """
    try:
        config = get_config()

        # Load settings.json for custom config
        settings_file = config.config_dir / "settings.json"
        max_size_gb = 10.0  # Default
        retention_days = 30  # Default
        min_disk_free_gb = 1.0  # Default

        if settings_file.exists():
            try:
                with open(settings_file, 'r') as f:
                    settings = json.load(f)
                    trash_config = settings.get('trash', {})
                    max_size_gb = trash_config.get('max_size_gb', max_size_gb)
                    retention_days = trash_config.get('retention_days', retention_days)
                    min_disk_free_gb = trash_config.get('min_disk_free_gb', min_disk_free_gb)
            except Exception as e:
                logger.warning(f"Failed to load trash config from settings: {e}")

        return TrashConfigResponse(
            max_size_gb=max_size_gb,
            retention_days=retention_days,
            min_disk_free_gb=min_disk_free_gb
        )

    except Exception as e:
        logger.error(f"Error getting trash config: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting trash config: {str(e)}"
        )


@router.post("/empty", response_model=EmptyTrashResponse)
async def empty_trash():
    """
    Empty the trash directory.

    Phase 19.5: Logs event to journal before deletion

    Returns:
        EmptyTrashResponse with result

    Example:
        POST /api/trash/empty
    """
    try:
        config = get_config()
        trash_dir = config.trash_dir

        if not trash_dir.exists():
            return EmptyTrashResponse(
                success=True,
                message="Trash is already empty",
                items_deleted=0
            )

        # Count items before deletion
        items_before = sum(1 for _ in trash_dir.rglob("*") if _.is_file())

        if items_before == 0:
            return EmptyTrashResponse(
                success=True,
                message="Trash is already empty",
                items_deleted=0
            )

        # Phase 19.5: Log to journal before deleting
        journal = JournalManager(config.config_dir)
        journal.log_event(
            action="trash_emptied",
            target="library",
            status="completed"
        )
        logger.info(f"Journal logged: trash_emptied (items: {items_before})")

        # Delete all items in trash
        import shutil
        for item in trash_dir.rglob("*"):
            if item.is_file():
                item.unlink()
            elif item.is_dir() and item != trash_dir:
                shutil.rmtree(item)

        logger.info(f"Trash emptied: {items_before} items deleted")

        return EmptyTrashResponse(
            success=True,
            message=f"Trash emptied successfully ({items_before} items deleted)",
            items_deleted=items_before
        )

    except Exception as e:
        logger.error(f"Error emptying trash: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error emptying trash: {str(e)}"
        )
