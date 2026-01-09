"""
Utilities API endpoints for Galgame Library Manager.

**THE UTILITY BELT: Manual Helper Tools**

Provides REST API for manual file operations:
- Reveal in Explorer (select file)
- Copy path to clipboard
- Open file with default app
- Extract archives (background task)
- Get task status
"""

import logging
from pathlib import Path
from typing import Dict, Any

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel, Field

from ..services.system import get_system_bridge
from ..services.archiver import get_archive_service
from ..config import get_config
from ..core.config import settings
from ..core.path_safety import is_safe_path

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/utils", tags=["utilities"])


def _resolve_library_path(path_value: str, library_root: Path, allow_external: bool) -> Path:
    """Resolve a path relative to library_root and enforce path safety."""
    candidate = Path(path_value)
    if not candidate.is_absolute():
        candidate = library_root / path_value
    if not allow_external and not is_safe_path(candidate, library_root):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Path is outside library root: {path_value}"
        )
    return candidate


# ============================================================================
# Pydantic Models
# ============================================================================

class RevealRequest(BaseModel):
    """Request model for reveal in Explorer."""
    path: str = Field(..., description="Path to file or folder to reveal")


class CopyRequest(BaseModel):
    """Request model for copy to clipboard."""
    text: str = Field(..., description="Text to copy to clipboard")


class OpenRequest(BaseModel):
    """Request model for open file."""
    path: str = Field(..., description="Path to file or folder to open")


class ExtractRequest(BaseModel):
    """Request model for archive extraction."""
    source_path: str = Field(..., description="Path to archive file (RAR/7Z/ZIP)")
    target_dir: str = Field(..., description="Target extraction directory")
    background: bool = Field(True, description="Run extraction in background (default: True)")


class TaskStatusResponse(BaseModel):
    """Response model for task status."""
    task_id: str
    source_path: str
    target_dir: str
    status: str
    progress: int
    current_file: str
    total_files: int
    error: str | None = None
    created_at: str
    started_at: str | None = None
    completed_at: str | None = None


# ============================================================================
# API Endpoints
# ============================================================================

@router.post("/reveal")
async def reveal_in_explorer(request: RevealRequest):
    """
    Reveal file/folder in Explorer and SELECT it.

    Windows: Opens Explorer and selects the file
    macOS: Opens Finder and selects the file
    Linux: Opens file manager and shows the file

    Args:
        request: RevealRequest with path

    Returns:
        Success message

    Example:
        POST /api/utils/reveal
        {
            "path": "D:/Games/Fate/Game/setup.exe"
        }
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]
        resolved_path = _resolve_library_path(request.path, library_root, settings.ALLOW_EXTERNAL_PATHS)

        bridge = get_system_bridge()
        result = bridge.reveal_in_explorer(str(resolved_path))

        if not result["success"]:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=result["message"]
            )

        return result

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error revealing in Explorer: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error revealing: {str(e)}"
        )


@router.post("/copy")
async def copy_to_clipboard(request: CopyRequest):
    """
    Copy text to clipboard.

    Copies the provided text to system clipboard.

    Args:
        request: CopyRequest with text

    Returns:
        Success message

    Example:
        POST /api/utils/copy
        {
            "text": "D:/Games/Fate/Game/setup.exe"
        }
    """
    try:
        bridge = get_system_bridge()
        result = bridge.copy_to_clipboard(request.text)

        if not result["success"]:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=result["message"]
            )

        return result

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error copying to clipboard: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error copying: {str(e)}"
        )


@router.post("/open")
async def open_file(request: OpenRequest):
    """
    Open file or folder with default application.

    Opens the file with its default associated application:
    - PDFs → Open in PDF viewer
    - EXE → Run installer or game
    - Folders → Open in file manager

    Args:
        request: OpenRequest with path

    Returns:
        Success message

    Example:
        POST /api/utils/open
        {
            "path": "D:/Games/Fate/Manual.pdf"
        }
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]
        resolved_path = _resolve_library_path(request.path, library_root, settings.ALLOW_EXTERNAL_PATHS)

        bridge = get_system_bridge()
        path_obj = str(resolved_path)

        # Check if it's a directory
        if Path(path_obj).is_dir():
            result = bridge.open_directory(path_obj)
        else:
            result = bridge.open_file(path_obj)

        if not result["success"]:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=result["message"]
            )

        return result

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error opening file: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error opening: {str(e)}"
        )


@router.post("/extract")
async def extract_archive(request: ExtractRequest):
    """
    Extract archive to target directory.

    Supports:
    - RAR archives (via 7-Zip)
    - 7Z archives (via 7-Zip)
    - ZIP archives (via 7-Zip or patool)
    - Multi-part archives (automatic detection)
    - Background execution with progress tracking

    Args:
        request: ExtractRequest with source_path, target_dir, background

    Returns:
        Dict with task_id (if background=True) or result (if background=False)

    Example:
        POST /api/utils/extract
        {
            "source_path": "D:/Repository/Fate.part1.rar",
            "target_dir": "D:/Games/Fate",
            "background": true
        }

    Response (background=true):
        {
            "success": true,
            "task_id": "extract_1234567890",
            "message": "Extraction started in background"
        }

    Response (background=false):
        {
            "success": true,
            "method": "7zip",
            "parts": 3,
            "message": "Extracted 3 parts successfully"
        }
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]
        target_dir = _resolve_library_path(request.target_dir, library_root, settings.ALLOW_EXTERNAL_PATHS)

        source_path = Path(request.source_path)
        if not source_path.is_absolute():
            source_path = library_root / request.source_path
        if not settings.ALLOW_EXTERNAL_PATHS and not is_safe_path(source_path, library_root):
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Path is outside library root: {request.source_path}"
            )

        archiver = get_archive_service()
        result = archiver.extract_archive(
            source_path=str(source_path),
            target_dir=str(target_dir),
            background=request.background
        )

        if "error" in result and result["error"]:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=result["error"]
            )

        return result

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error extracting archive: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error extracting: {str(e)}"
        )


@router.get("/tasks/{task_id}", response_model=TaskStatusResponse)
async def get_task_status(task_id: str):
    """
    Get status of background extraction task.

    Args:
        task_id: Task identifier (returned from /extract endpoint)

    Returns:
        TaskStatusResponse with current task status

    Example:
        GET /api/utils/tasks/extract_1234567890

    Response:
        {
            "task_id": "extract_1234567890",
            "source_path": "D:/Repository/Fate.part1.rar",
            "target_dir": "D:/Games/Fate",
            "status": "running",
            "progress": 45,
            "current_file": "Game/data.xp3",
            "total_files": 150,
            "error": null,
            "created_at": "2026-01-02T12:00:00",
            "started_at": "2026-01-02T12:00:01",
            "completed_at": null
        }
    """
    try:
        archiver = get_archive_service()
        task = archiver.get_task_status(task_id)

        if not task:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Task not found: {task_id}"
            )

        return TaskStatusResponse(**task)

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error getting task status: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting task status: {str(e)}"
        )


@router.get("/status")
async def get_utility_status() -> Dict[str, Any]:
    """
    Get status of utility services.

    Returns information about:
    - 7-Zip availability
    - patool availability
    - Active extraction tasks

    Returns:
        Dict with service status

    Example:
        GET /api/utils/status

    Response:
        {
            "seven_zip_available": true,
            "seven_zip_path": "C:\\Program Files\\7-Zip\\7z.exe",
            "patool_available": true,
            "active_tasks": 2,
            "supported_formats": ["rar", "7z", "zip", "iso", "tar", "gz"]
        }
    """
    try:
        archiver = get_archive_service()

        # Check supported formats
        supported_formats = []
        if archiver.seven_zip_path:
            # 7-Zip supports: RAR, 7Z, ZIP, ISO, TAR, GZ, etc.
            supported_formats = ["rar", "7z", "zip", "iso", "tar", "gz", "bz2", "xz", "cab"]
        else:
            # patool supports: ZIP, TAR, GZ, BZ2, XZ (limited)
            supported_formats = ["zip", "tar", "gz", "bz2", "xz"]

        return {
            "seven_zip_available": archiver.seven_zip_path is not None,
            "seven_zip_path": archiver.seven_zip_path,
            "patool_available": True,  # We can import it
            "use_patool": archiver.use_patool,
            "active_tasks": len(archiver.tasks),
            "supported_formats": supported_formats
        }

    except Exception as e:
        logger.error(f"Error getting utility status: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting status: {str(e)}"
        )
