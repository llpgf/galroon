"""
System API endpoints for Galgame Library Manager.

**PHASE 10: Safe Deletion**

Provides secure deletion using send2trash (moves to OS trash instead of permanent delete).
"""

import logging
from pathlib import Path
from typing import Dict, Any

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel, Field

from send2trash import send2trash

from ..core.path_safety import is_safe_path
from ..config import get_config
from ..core.config import settings

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/system", tags=["system"])


# ============================================================================
# Pydantic Models
# ============================================================================

class DeleteRequest(BaseModel):
    """Request model for safe deletion."""
    path: str = Field(..., description="Path to file/folder to delete")
    library_root: str = Field(None, description="Library root for safety check (optional)")


class DeleteResponse(BaseModel):
    """Response model for deletion result."""
    success: bool
    path: str
    message: str
    operation: str  # "trash" or "delete"


class OpenFolderRequest(BaseModel):
    """Request model for opening folder in file manager."""
    path: str = Field(..., description="Path to folder to open")


class OpenFolderResponse(BaseModel):
    """Response model for folder open result."""
    success: bool
    path: str
    message: str


# ============================================================================
# API Endpoints
# ============================================================================

@router.post("/delete", response_model=DeleteResponse)
async def safe_delete(request: DeleteRequest):
    """
    Safely delete a file or folder by moving it to the OS trash.

    Uses send2trash library to move files to the system trash instead of
    permanent deletion. This is much safer than os.remove() or shutil.rmtree().

    Args:
        request: DeleteRequest with path and optional library_root

    Returns:
        DeleteResponse with result
    """
    try:
        config = get_config()
        library_root = Path(request.library_root) if request.library_root else config.library_roots[0]
        target_path = Path(request.path)
        if not target_path.is_absolute():
            target_path = library_root / request.path

        # Validate path exists
        if not target_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Path not found: {target_path}"
            )

        # Safety check: Validate path is safe
        if not settings.ALLOW_EXTERNAL_PATHS and not is_safe_path(target_path, library_root):
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Path is outside library root: {target_path}"
            )

        # Perform safe deletion (move to trash)
        try:
            send2trash(str(target_path))

            logger.info(f"Moved to trash: {target_path}")

            return DeleteResponse(
                success=True,
                path=str(target_path),
                message=f"Moved to trash: {target_path.name}",
                operation="trash"
            )

        except Exception as e:
            logger.error(f"Error moving to trash: {e}")

            # Fallback to permanent delete if trash fails
            try:
                if target_path.is_file():
                    target_path.unlink()
                else:
                    import shutil
                    shutil.rmtree(target_path)

                logger.warning(f"Permanent delete (trash failed): {target_path}")

                return DeleteResponse(
                    success=True,
                    path=str(target_path),
                    message=f"Permanently deleted (trash unavailable): {target_path.name}",
                    operation="delete"
                )
            except Exception as e2:
                raise HTTPException(
                    status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                    detail=f"Failed to delete: {str(e2)}"
                )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error in safe_delete: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error deleting file: {str(e)}"
        )


@router.post("/open_folder", response_model=OpenFolderResponse)
async def open_folder(request: OpenFolderRequest):
    """
    Open a folder in the system's default file manager.

    Args:
        request: OpenFolderRequest with path

    Returns:
        OpenFolderResponse with result
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]
        folder_path = Path(request.path)
        if not folder_path.is_absolute():
            folder_path = library_root / request.path

        if not settings.ALLOW_EXTERNAL_PATHS and not is_safe_path(folder_path, library_root):
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Path is outside library root: {folder_path}"
            )

        # Validate path exists
        if not folder_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Folder not found: {folder_path}"
            )

        if not folder_path.is_dir():
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"Path is not a directory: {folder_path}"
            )

        # Open in file manager (platform-specific)
        import platform
        import subprocess

        system = platform.system()

        try:
            if system == "Windows":
                # Windows: use explorer
                subprocess.run(["explorer", str(folder_path)])
            elif system == "Darwin":  # macOS
                # macOS: use open
                subprocess.run(["open", str(folder_path)])
            else:  # Linux and others
                # Linux: use xdg-open
                subprocess.run(["xdg-open", str(folder_path)])

            logger.info(f"Opened folder: {folder_path}")

            return OpenFolderResponse(
                success=True,
                path=str(folder_path),
                message=f"Opened folder: {folder_path.name}"
            )

        except FileNotFoundError:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="File manager not found. Please install a file manager."
            )
        except Exception as e:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail=f"Failed to open folder: {str(e)}"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error opening folder: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error opening folder: {str(e)}"
        )


@router.get("/info")
async def get_system_info() -> Dict[str, Any]:
    """
    Get system information and paths.

    Returns:
        Dictionary with system info
    """
    try:
        config = get_config()

        import platform
        import shutil

        # Get disk usage for library roots
        roots_info = []
        for root in config.library_roots:
            try:
                usage = shutil.disk_usage(root)
                roots_info.append({
                    "path": str(root),
                    "exists": root.exists(),
                    "total_gb": round(usage.total / (1024**3), 2),
                    "used_gb": round(usage.used / (1024**3), 2),
                    "free_gb": round(usage.free / (1024**3), 2),
                })
            except Exception as e:
                roots_info.append({
                    "path": str(root),
                    "exists": root.exists(),
                    "error": str(e)
                })

        return {
            "platform": platform.system(),
            "platform_release": platform.release(),
            "python_version": platform.python_version(),
            "library_roots": [str(r) for r in config.library_roots],
            "roots_info": roots_info,
            "config_dir": str(config.config_dir),
            "trash_dir": str(config.trash_dir),
        }

    except Exception as e:
        logger.error(f"Error getting system info: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting system info: {str(e)}"
        )
