"""
Curator API endpoints for Galgame Library Manager.

**PHASE 10: The Curator Backend**

Provides REST API endpoints for manual game management:
- Manual VNDB ID identification
- Field locking/unlocking
- Extras browsing
- Version management
"""

import logging
from pathlib import Path
from typing import Dict, Any, List, Optional

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel, Field

from ..metadata.curator import Curator, get_curator
from ..metadata.models import UnifiedMetadata
from ..config import get_config
from ..core.path_safety import is_safe_path

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/curator", tags=["curator"])


def _resolve_folder_path(folder_path: str, library_root: Path) -> Path:
    """Resolve a folder path relative to library_root and enforce path safety."""
    candidate = Path(folder_path)
    if not candidate.is_absolute():
        candidate = library_root / folder_path
    if not is_safe_path(candidate, library_root):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Path is outside library root"
        )
    return candidate


def _resolve_child_path(parent_dir: Path, child_path: str) -> Path:
    """Resolve a child path relative to parent_dir and enforce path safety."""
    candidate = parent_dir / child_path
    if not is_safe_path(candidate, parent_dir):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Path is outside game folder"
        )
    return candidate


# ============================================================================
# Pydantic Models
# ============================================================================

class IdentifyRequest(BaseModel):
    """Request model for manual game identification."""
    folder_path: str = Field(..., description="Path to game folder")
    vndb_id: str = Field(..., description="VNDB ID to link (e.g., v12345)")
    fetch_metadata: bool = Field(True, description="Whether to fetch fresh metadata")


class IdentifyResponse(BaseModel):
    """Response model for identification result."""
    success: bool
    folder_path: str
    vndb_id: str
    metadata: Optional[Dict[str, Any]] = None
    merged: bool = False
    message: str


class LockFieldsRequest(BaseModel):
    """Request model for locking fields."""
    folder_path: str = Field(..., description="Path to game folder")
    field_names: List[str] = Field(..., description="List of field names to lock")


class LockFieldsResponse(BaseModel):
    """Response model for field locking."""
    success: bool
    locked_count: int
    locked_fields: List[str]
    message: str


class UpdateFieldRequest(BaseModel):
    """Request model for updating a single field."""
    folder_path: str = Field(..., description="Path to game folder")
    field_name: str = Field(..., description="Field name to update")
    value: Any = Field(..., description="New value")
    lock_after_update: bool = Field(False, description="Lock field after update")


class UpdateFieldResponse(BaseModel):
    """Response model for field update."""
    success: bool
    field_name: str
    old_value: Any = None
    new_value: Any = None
    locked: bool = False
    message: str


class ExtraFile(BaseModel):
    """Represents an extra file."""
    name: str
    path: str
    type: str  # pdf, jpg, png, mp3, flac, etc.
    category: str  # manual, artbook, ost, save, etc.
    size: int
    modified_time: float


class ExtrasResponse(BaseModel):
    """Response model for extras listing."""
    folder_path: str
    extras: List[ExtraFile]
    total_count: int
    total_size_mb: float
    categories: Dict[str, int]


class MergeVersionsRequest(BaseModel):
    """Request model for merging versions."""
    vndb_id: str = Field(..., description="VNDB ID to merge")
    primary_folder: Optional[str] = Field(None, description="Optional primary folder path")


class MergeVersionsResponse(BaseModel):
    """Response model for version merge."""
    success: bool
    vndb_id: str
    folders_found: int
    folders_updated: int
    message: str


# ============================================================================
# PHASE 18.5: Custom User Tags
# ============================================================================

class UpdateTagsRequest(BaseModel):
    """Request model for updating user tags."""
    folder_path: str = Field(..., description="Path to game folder")
    user_tags: List[str] = Field(..., description="New list of user tags")


class UpdateTagsResponse(BaseModel):
    """Response model for updating user tags."""
    success: bool
    folder_path: str
    user_tags: List[str]
    message: str


# ============================================================================
# API Endpoints
# ============================================================================

@router.post("/identify", response_model=IdentifyResponse)
async def identify_game(request: IdentifyRequest):
    """
    Manually identify a game by linking it to a VNDB ID.

    This will:
    1. Fetch metadata from VNDB using the provided ID
    2. Detect assets in the folder
    3. Add/update the folder as a version
    4. Merge if the VNDB ID already exists in another folder

    Args:
        request: IdentifyRequest with folder_path, vndb_id, fetch_metadata

    Returns:
        IdentifyResponse with result
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]  # Use first root

        curator = get_curator(library_root)
        resolved_folder = _resolve_folder_path(request.folder_path, library_root)

        result = curator.identify_game(
            folder_path=resolved_folder,
            vndb_id=request.vndb_id,
            fetch_metadata=request.fetch_metadata
        )

        # Phase 19.5: Log metadata application to journal
        if result.success:
            from ..core.journal import JournalManager
            journal = JournalManager(config.config_dir)
            journal.log_event(
                action="metadata_applied",
                target=f"game/{result.vndb_id}",
                status="completed"
            )
            logger.info(f"Journal logged: metadata_applied to {result.vndb_id}")

        return IdentifyResponse(
            success=result.success,
            folder_path=str(result.folder_path),
            vndb_id=result.vndb_id,
            metadata=result.metadata.model_dump() if result.metadata else None,
            merged=result.merged,
            message=result.message
        )

    except Exception as e:
        logger.error(f"Error identifying game: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error identifying game: {str(e)}"
        )


@router.post("/lock_fields", response_model=LockFieldsResponse)
async def lock_fields(request: LockFieldsRequest):
    """
    Lock multiple metadata fields to prevent overwriting.

    Args:
        request: LockFieldsRequest with folder_path and field_names

    Returns:
        LockFieldsResponse with result
    """
    try:
        from ..metadata.manager import get_resource_manager

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        folder_path = _resolve_folder_path(request.folder_path, library_root)

        # Load metadata
        metadata_dict = resource_manager.load_metadata(folder_path)
        if not metadata_dict:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"No metadata found for folder: {folder_path}"
            )

        metadata = UnifiedMetadata(**metadata_dict)

        # Lock fields
        locked_count = metadata.lock_fields(request.field_names)

        # Save metadata
        success = resource_manager.save_metadata(metadata.model_dump(), folder_path)

        if success:
            return LockFieldsResponse(
                success=True,
                locked_count=locked_count,
                locked_fields=metadata.get_locked_fields(),
                message=f"Locked {locked_count} fields"
            )
        else:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Failed to save metadata"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error locking fields: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error locking fields: {str(e)}"
        )


@router.post("/unlock_fields", response_model=LockFieldsResponse)
async def unlock_fields(request: LockFieldsRequest):
    """
    Unlock multiple metadata fields to allow updates.

    Args:
        request: LockFieldsRequest with folder_path and field_names

    Returns:
        LockFieldsResponse with result
    """
    try:
        from ..metadata.manager import get_resource_manager

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        folder_path = _resolve_folder_path(request.folder_path, library_root)

        # Load metadata
        metadata_dict = resource_manager.load_metadata(folder_path)
        if not metadata_dict:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"No metadata found for folder: {folder_path}"
            )

        metadata = UnifiedMetadata(**metadata_dict)

        # Unlock fields
        unlocked_count = metadata.unlock_fields(request.field_names)

        # Save metadata
        success = resource_manager.save_metadata(metadata.model_dump(), folder_path)

        if success:
            return LockFieldsResponse(
                success=True,
                locked_count=unlocked_count,
                locked_fields=metadata.get_locked_fields(),
                message=f"Unlocked {unlocked_count} fields"
            )
        else:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Failed to save metadata"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error unlocking fields: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error unlocking fields: {str(e)}"
        )


@router.post("/update_field", response_model=UpdateFieldResponse)
async def update_field(request: UpdateFieldRequest):
    """
    Update a single metadata field and optionally lock it.

    Args:
        request: UpdateFieldRequest with folder_path, field_name, value, lock_after_update

    Returns:
        UpdateFieldResponse with result
    """
    try:
        from ..metadata.manager import get_resource_manager

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        folder_path = _resolve_folder_path(request.folder_path, library_root)

        # Load metadata
        metadata_dict = resource_manager.load_metadata(folder_path)
        if not metadata_dict:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"No metadata found for folder: {folder_path}"
            )

        metadata = UnifiedMetadata(**metadata_dict)

        # Get old value
        if not hasattr(metadata, request.field_name):
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"Invalid field name: {request.field_name}"
            )

        field = getattr(metadata, request.field_name)

        # Handle MetadataField wrapper
        if hasattr(field, 'value'):
            old_value = field.value
            field.value = request.value
            field.source = "manual"
        else:
            old_value = getattr(metadata, request.field_name)
            setattr(metadata, request.field_name, request.value)

        # Lock field if requested
        if request.lock_after_update:
            metadata.lock_fields([request.field_name])

        # Save metadata
        success = resource_manager.save_metadata(metadata.model_dump(), folder_path)

        if success:
            return UpdateFieldResponse(
                success=True,
                field_name=request.field_name,
                old_value=old_value,
                new_value=request.value,
                locked=request.lock_after_update,
                message=f"Updated {request.field_name}"
            )
        else:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Failed to save metadata"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error updating field: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error updating field: {str(e)}"
        )


@router.get("/extras/{folder_path:path}", response_model=ExtrasResponse)
async def get_extras(folder_path: str):
    """
    Scan the Extras/ and Repository/ subdirectories for extra content.

    Returns a structured list of extra files organized by type.

    Args:
        folder_path: Path to game folder (relative to library root)

    Returns:
        ExtrasResponse with list of extra files
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]

        # Resolve full path
        full_path = _resolve_folder_path(folder_path, library_root)

        if not full_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Folder not found: {folder_path}"
            )

        extras = []
        categories: Dict[str, int] = {}
        total_size = 0

        # Define directories to scan (from Phase 9.5 Standard)
        extra_dirs = ["Extras", "Repository"]

        # File type mappings
        type_extensions = {
            ".pdf": "pdf",
            ".jpg": "image",
            ".jpeg": "image",
            ".png": "image",
            ".webp": "image",
            ".mp3": "audio",
            ".flac": "audio",
            ".wav": "audio",
            ".txt": "text",
            ".nfo": "text",
        }

        # Category patterns
        category_patterns = {
            "manual": ["manual", "guide", "walkthrough"],
            "artbook": ["artbook", "art.*book", "gallery"],
            "ost": ["ost", "soundtrack", "bgm"],
            "save": ["save", "savestate", "savedata"],
        }

        for extra_dir in extra_dirs:
            dir_path = full_path / extra_dir

            if not dir_path.exists():
                continue

            # Scan directory
            for file_path in dir_path.rglob("*"):
                if not file_path.is_file():
                    continue

                try:
                    # Get file info
                    stat = file_path.stat()
                    file_size = stat.st_size
                    total_size += file_size

                    # Determine file type
                    file_ext = file_path.suffix.lower()
                    file_type = type_extensions.get(file_ext, "unknown")

                    # Determine category
                    category = "other"
                    file_name_lower = file_path.name.lower()

                    for cat_name, patterns in category_patterns.items():
                        for pattern in patterns:
                            if pattern in file_name_lower:
                                category = cat_name
                                break
                        if category != "other":
                            break

                    # Count category
                    categories[category] = categories.get(category, 0) + 1

                    extra = ExtraFile(
                        name=file_path.name,
                        path=str(file_path.relative_to(library_root)),
                        type=file_type,
                        category=category,
                        size=file_size,
                        modified_time=stat.st_mtime
                    )
                    extras.append(extra)

                except Exception as e:
                    logger.warning(f"Error processing file {file_path}: {e}")

        return ExtrasResponse(
            folder_path=folder_path,
            extras=extras,
            total_count=len(extras),
            total_size_mb=round(total_size / (1024 * 1024), 2),
            categories=categories
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error getting extras: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting extras: {str(e)}"
        )


@router.post("/merge_versions", response_model=MergeVersionsResponse)
async def merge_versions(request: MergeVersionsRequest):
    """
    Merge all versions of a work under one VNDB ID.

    Ensures all folders with the same VNDB ID have consistent metadata.

    Args:
        request: MergeVersionsRequest with vndb_id and optional primary_folder

    Returns:
        MergeVersionsResponse with merge result
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]

        curator = get_curator(library_root)

        primary_folder = (
            _resolve_folder_path(request.primary_folder, library_root)
            if request.primary_folder
            else None
        )

        result = curator.merge_versions(
            vndb_id=request.vndb_id,
            primary_folder=primary_folder
        )

        return MergeVersionsResponse(
            success=result["success"],
            vndb_id=result["vndb_id"],
            folders_found=result["folders_found"],
            folders_updated=result["folders_updated"],
            message=result["message"]
        )

    except Exception as e:
        logger.error(f"Error merging versions: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error merging versions: {str(e)}"
        )


@router.patch("/games/tags", response_model=UpdateTagsResponse)
async def update_game_tags(request: UpdateTagsRequest):
    """
    Update user-defined tags for a game.

    PHASE 18.5: Allows personal organization with custom tags.
    Unlike provider tags (from VNDB), these are fully user-editable.

    Args:
        request: UpdateTagsRequest with folder_path and user_tags list

    Returns:
        UpdateTagsResponse with updated user tags
    """
    try:
        from ..metadata.manager import get_resource_manager

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        folder_path = _resolve_folder_path(request.folder_path, library_root)

        # Load metadata
        metadata_dict = resource_manager.load_metadata(folder_path)
        if not metadata_dict:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"No metadata found for folder: {folder_path}"
            )

        metadata = UnifiedMetadata(**metadata_dict)

        # Update user tags
        metadata.user_tags = request.user_tags

        # Save metadata
        success = resource_manager.save_metadata(metadata.model_dump(), folder_path)

        if success:
            logger.info(f"Updated user tags for {folder_path}: {request.user_tags}")
            return UpdateTagsResponse(
                success=True,
                folder_path=str(folder_path),
                user_tags=metadata.user_tags,
                message=f"Updated {len(metadata.user_tags)} user tags"
            )
        else:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Failed to save metadata"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error updating user tags: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error updating user tags: {str(e)}"
        )


# ============================================================================
# PHASE 19.6: Version Manager APIs
# ============================================================================

class GameVersionInfo(BaseModel):
    """Response model for a game version."""
    id: str = Field(..., description="Version ID (path-based)")
    path: str = Field(..., description="Full path to version folder")
    label: str = Field(..., description="Version label")
    is_primary: bool = Field(..., description="Whether this is the primary version")
    assets: List[str] = Field(..., description="Asset tags")


class VersionsListResponse(BaseModel):
    """Response model for versions list."""
    vndb_id: str
    versions: List[GameVersionInfo]
    total_count: int


class AddVersionRequest(BaseModel):
    """Request model for adding a version."""
    vndb_id: str = Field(..., description="VNDB ID")
    folder_path: str = Field(..., description="Path to folder to add as version")
    label: str = Field("", description="Optional version label")


class AddVersionResponse(BaseModel):
    """Response model for adding a version."""
    success: bool
    vndb_id: str
    version_id: str
    message: str


class SetPrimaryVersionResponse(BaseModel):
    """Response model for setting primary version."""
    success: bool
    vndb_id: str
    primary_version_id: str
    message: str


class DeleteVersionResponse(BaseModel):
    """Response model for deleting a version."""
    success: bool
    vndb_id: str
    deleted_version_id: str
    remaining_count: int
    message: str


@router.get("/games/{vndb_id}/versions", response_model=VersionsListResponse)
async def get_game_versions(vndb_id: str):
    """
    Get all versions (installations) for a game.

    Args:
        vndb_id: VNDB ID (e.g., v12345)

    Returns:
        VersionsListResponse with list of all versions
    """
    try:
        from ..metadata.manager import get_resource_manager

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        # Find all metadata files with this vndb_id
        versions = []
        for meta_file in library_root.rglob("galgame_metadata.json"):
            try:
                metadata_dict = resource_manager.load_metadata(meta_file.parent)
                if not metadata_dict:
                    continue

                metadata = UnifiedMetadata(**metadata_dict)

                # Check if this metadata has the target vndb_id
                if metadata.vndb_id == vndb_id:
                    for version in metadata.versions:
                        versions.append(GameVersionInfo(
                            id=version.path,
                            path=version.path,
                            label=version.label,
                            is_primary=version.is_primary,
                            assets=version.assets
                        ))
            except Exception as e:
                logger.warning(f"Error reading metadata from {meta_file}: {e}")

        return VersionsListResponse(
            vndb_id=vndb_id,
            versions=versions,
            total_count=len(versions)
        )

    except Exception as e:
        logger.error(f"Error getting versions for {vndb_id}: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting versions: {str(e)}"
        )


@router.post("/games/{vndb_id}/versions", response_model=AddVersionResponse)
async def add_game_version(vndb_id: str, request: AddVersionRequest):
    """
    Add a folder path as a new version to this game.

    Args:
        vndb_id: VNDB ID
        request: AddVersionRequest with folder_path and optional label

    Returns:
        AddVersionResponse with result
    """
    try:
        from ..metadata.manager import get_resource_manager

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        folder_path = _resolve_folder_path(request.folder_path, library_root)

        # Validate folder exists
        if not folder_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Folder not found: {folder_path}"
            )

        # Load or create metadata for this folder
        metadata_dict = resource_manager.load_metadata(folder_path)
        if metadata_dict:
            metadata = UnifiedMetadata(**metadata_dict)
        else:
            metadata = create_empty_metadata()

        # Set vndb_id
        metadata.vndb_id = vndb_id

        # Add as version if not already there
        version_exists = any(v.path == str(folder_path) for v in metadata.versions)

        if not version_exists:
            metadata.add_version(
                path=str(folder_path),
                label=request.label,
                is_primary=False
            )

            # Save metadata
            success = resource_manager.save_metadata(metadata.model_dump(), folder_path)

            if success:
                logger.info(f"Added version {folder_path} to {vndb_id}")
                return AddVersionResponse(
                    success=True,
                    vndb_id=vndb_id,
                    version_id=str(folder_path),
                    message=f"Added version: {request.label or folder_path.name}"
                )
            else:
                raise HTTPException(
                    status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                    detail="Failed to save metadata"
                )
        else:
            return AddVersionResponse(
                success=True,
                vndb_id=vndb_id,
                version_id=str(folder_path),
                message="Version already exists"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error adding version: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error adding version: {str(e)}"
        )


@router.patch("/games/{vndb_id}/versions/{version_id}/primary", response_model=SetPrimaryVersionResponse)
async def set_primary_version(vndb_id: str, version_id: str):
    """
    Set a specific version as the primary version.

    Only one version can be primary at a time.

    Args:
        vndb_id: VNDB ID
        version_id: Version ID (folder path)

    Returns:
        SetPrimaryVersionResponse with result
    """
    try:
        from ..metadata.manager import get_resource_manager

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        # Find and update all metadata files with this vndb_id
        updated_count = 0

        for meta_file in library_root.rglob("galgame_metadata.json"):
            try:
                metadata_dict = resource_manager.load_metadata(meta_file.parent)
                if not metadata_dict:
                    continue

                metadata = UnifiedMetadata(**metadata_dict)

                if metadata.vndb_id == vndb_id:
                    # Update primary status
                    for version in metadata.versions:
                        version.is_primary = (version.path == version_id)

                    # Save metadata
                    success = resource_manager.save_metadata(metadata.model_dump(), meta_file.parent)
                    if success:
                        updated_count += 1

            except Exception as e:
                logger.warning(f"Error updating metadata in {meta_file}: {e}")

        if updated_count > 0:
            logger.info(f"Set {version_id} as primary for {vndb_id}")
            return SetPrimaryVersionResponse(
                success=True,
                vndb_id=vndb_id,
                primary_version_id=version_id,
                message=f"Set as primary version"
            )
        else:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"No versions found for {vndb_id}"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error setting primary version: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error setting primary version: {str(e)}"
        )


# ============================================================================
# PHASE 24.0: THE CURATOR - Smart Merge & Image Management
# ============================================================================

class SmartIdentifyRequest(BaseModel):
    """Request model for smart identification with field locking awareness."""
    folder_path: str = Field(..., description="Path to game folder")
    vndb_id: str = Field(..., description="VNDB ID to fetch and merge")
    preserve_locked: bool = Field(True, description="Preserve locked fields during merge")


class SmartIdentifyResponse(BaseModel):
    """Response model for smart identification."""
    success: bool
    vndb_id: str
    fields_updated: List[str] = []
    fields_skipped: List[str] = []  # Locked fields
    message: str


class ImageSelectRequest(BaseModel):
    """Request model for selecting cover/background images."""
    folder_path: str = Field(..., description="Path to game folder")
    image_path: str = Field(..., description="Path to selected image (relative to game folder)")
    image_type: str = Field(..., description="Type: 'cover' or 'background'")
    create_symlink: bool = Field(False, description="Create cover.jpg symlink (or just update metadata)")


class ImageSelectResponse(BaseModel):
    """Response model for image selection."""
    success: bool
    image_type: str
    image_path: str
    cover_path: Optional[str] = None
    message: str


class ImageListResponse(BaseModel):
    """Response model for listing images in game folder."""
    folder_path: str
    images: List[Dict[str, Any]]  # List of {path, name, type, url}
    total_count: int
    message: str


@router.post("/games/{folder_path:path}/identify", response_model=SmartIdentifyResponse)
async def smart_identify_game(folder_path: str, request: SmartIdentifyRequest):
    """
    Smart identify game with field locking awareness.

    Phase 24.0: Performs a "Smart Merge" where:
    - Unlocked fields are overwritten with VNDB data
    - Locked fields are preserved (user edits are sacred)
    - Dual write: Updates both metadata.json and library.db

    Args:
        folder_path: Path to game folder (URL encoded)
        request: SmartIdentifyRequest with vndb_id and preserve_locked flag

    Returns:
        SmartIdentifyResponse with merge results
    """
    try:
        from ..metadata.manager import get_resource_manager
        from ..core.database import get_database
        from ..connectors.vndb import VNDBConnector

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        # Resolve full path
        full_path = _resolve_folder_path(folder_path, library_root)

        if not full_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Folder not found: {folder_path}"
            )

        # Load current metadata
        metadata_dict = resource_manager.load_metadata(full_path)
        if not metadata_dict:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"No metadata found for folder: {folder_path}"
            )

        metadata = UnifiedMetadata(**metadata_dict)

        # Track locked fields
        locked_fields = metadata.get_locked_fields()
        fields_updated = []
        fields_skipped = []

        # Fetch from VNDB
        vndb = VNDBConnector()
        vndb_metadata = await vndb.fetch_metadata(request.vndb_id)

        if not vndb_metadata:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Failed to fetch metadata from VNDB for ID: {request.vndb_id}"
            )

        # Smart merge: Update only unlocked fields
        for field_name, new_value in vndb_metadata.items():
            if field_name in locked_fields and request.preserve_locked:
                fields_skipped.append(field_name)
                logger.info(f"Skipping locked field: {field_name}")
                continue

            # Update unlocked field
            if hasattr(metadata, field_name):
                old_field = getattr(metadata, field_name)

                # Handle MetadataField wrapper
                if hasattr(old_field, 'value'):
                    old_field.value = new_value
                    old_field.source = "vndb"
                else:
                    setattr(metadata, field_name, new_value)

                fields_updated.append(field_name)
                logger.info(f"Updated field: {field_name}")

        # Set VNDB ID
        metadata.vndb_id = request.vndb_id

        # Dual write: 1. Save to metadata.json
        success_json = resource_manager.save_metadata(metadata.model_dump(), full_path)

        # Dual write: 2. Update library.db
        folder_mtime = full_path.stat().st_mtime
        json_path = full_path / 'metadata.json'
        json_mtime = json_path.stat().st_mtime if json_path.exists() else folder_mtime

        db = get_database()
        db.upsert_game(metadata.model_dump(), full_path, folder_mtime, json_mtime)

        if success_json:
            logger.info(
                f"Smart identify complete: {len(fields_updated)} updated, "
                f"{len(fields_skipped)} skipped (locked)"
            )

            return SmartIdentifyResponse(
                success=True,
                vndb_id=request.vndb_id,
                fields_updated=fields_updated,
                fields_skipped=fields_skipped,
                message=f"Updated {len(fields_updated)} fields, preserved {len(fields_skipped)} locked fields"
            )
        else:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Failed to save metadata"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error in smart identify: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error in smart identify: {str(e)}"
        )


@router.patch("/games/{folder_path:path}")
async def update_game_metadata(
    folder_path: str,
    metadata_update: Dict[str, Any],
    lock_fields: List[str] = []
):
    """
    Update game metadata with field locking support.

    Phase 24.0: Updates specific fields and optionally locks them.
    Dual write to metadata.json and library.db.

    Args:
        folder_path: Path to game folder (URL encoded)
        metadata_update: Dictionary of field names and values to update
        lock_fields: Optional list of fields to lock after update

    Returns:
        Dict with update results
    """
    try:
        from ..metadata.manager import get_resource_manager
        from ..core.database import get_database

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        # Resolve full path
        full_path = _resolve_folder_path(folder_path, library_root)

        if not full_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Folder not found: {folder_path}"
            )

        # Load current metadata
        metadata_dict = resource_manager.load_metadata(full_path)
        if not metadata_dict:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"No metadata found for folder: {folder_path}"
            )

        metadata = UnifiedMetadata(**metadata_dict)

        # Update fields
        updated_fields = []
        for field_name, new_value in metadata_update.items():
            if hasattr(metadata, field_name):
                field = getattr(metadata, field_name)

                # Handle MetadataField wrapper
                if hasattr(field, 'value'):
                    field.value = new_value
                    field.source = "manual"
                else:
                    setattr(metadata, field_name, new_value)

                updated_fields.append(field_name)

        # Lock fields if requested
        if lock_fields:
            metadata.lock_fields(lock_fields)

        # Dual write: 1. Save to metadata.json
        success_json = resource_manager.save_metadata(metadata.model_dump(), full_path)

        # Dual write: 2. Update library.db
        folder_mtime = full_path.stat().st_mtime
        json_path = full_path / 'metadata.json'
        json_mtime = json_path.stat().st_mtime if json_path.exists() else folder_mtime

        db = get_database()
        db.upsert_game(metadata.model_dump(), full_path, folder_mtime, json_mtime)

        if success_json:
            return {
                "success": True,
                "folder_path": folder_path,
                "updated_fields": updated_fields,
                "locked_fields": lock_fields,
                "message": f"Updated {len(updated_fields)} fields"
            }
        else:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Failed to save metadata"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error updating metadata: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error updating metadata: {str(e)}"
        )


@router.post("/games/{folder_path:path}/images/select", response_model=ImageSelectResponse)
async def select_game_image(folder_path: str, request: ImageSelectRequest):
    """
    Select an image as cover or background for a game.

    Phase 24.0: Updates metadata to point to selected image.
    Optionally creates a cover.jpg symlink for convenience.

    Args:
        folder_path: Path to game folder (URL encoded)
        request: ImageSelectRequest with image_path, image_type, create_symlink

    Returns:
        ImageSelectResponse with selection result
    """
    try:
        from ..metadata.manager import get_resource_manager
        from ..core.database import get_database

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        # Resolve full path
        full_path = _resolve_folder_path(folder_path, library_root)

        if not full_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Folder not found: {folder_path}"
            )

        # Load current metadata
        metadata_dict = resource_manager.load_metadata(full_path)
        if not metadata_dict:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"No metadata found for folder: {folder_path}"
            )

        metadata = UnifiedMetadata(**metadata_dict)

        # Resolve image path (relative to game folder)
        image_path = _resolve_child_path(full_path, request.image_path)

        # Validate image exists
        if not image_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Image not found: {request.image_path}"
            )

        cover_path = None

        if request.image_type == 'cover':
            # Update cover_path in metadata
            if hasattr(metadata, 'cover_path'):
                metadata.cover_path = str(image_path)
            if hasattr(metadata, 'cover_url'):
                # Update cover_url to point to local file
                metadata.cover_url = {
                    "value": str(image_path),
                    "source": "manual",
                    "locked": False
                }

            cover_path = str(image_path)

            # Optionally create cover.jpg symlink
            if request.create_symlink:
                cover_symlink = full_path / 'cover.jpg'
                # Remove existing symlink if present
                if cover_symlink.exists() or cover_symlink.is_symlink():
                    cover_symlink.unlink()

                # Create new symlink
                try:
                    cover_symlink.symlink_to(image_path)
                    cover_path = str(cover_symlink)
                    logger.info(f"Created cover.jpg symlink -> {image_path}")
                except OSError as e:
                    logger.warning(f"Failed to create symlink: {e}")
                    # Fallback: copy file
                    import shutil
                    shutil.copy2(image_path, cover_symlink)
                    cover_path = str(cover_symlink)
                    logger.info(f"Cover image copied to cover.jpg")

        elif request.image_type == 'background':
            # Add to metadata (would need to extend schema)
            # For now, just acknowledge
            logger.info(f"Background image selected: {image_path}")
        else:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"Invalid image_type: {request.image_type}. Must be 'cover' or 'background'"
            )

        # Save metadata
        success = resource_manager.save_metadata(metadata.model_dump(), full_path)

        # Update database
        folder_mtime = full_path.stat().st_mtime
        json_path = full_path / 'metadata.json'
        json_mtime = json_path.stat().st_mtime if json_path.exists() else folder_mtime

        db = get_database()
        db.upsert_game(metadata.model_dump(), full_path, folder_mtime, json_mtime)

        if success:
            return ImageSelectResponse(
                success=True,
                image_type=request.image_type,
                image_path=request.image_path,
                cover_path=cover_path,
                message=f"Selected as {request.image_type}"
            )
        else:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Failed to save metadata"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error selecting image: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error selecting image: {str(e)}"
        )


@router.get("/games/{folder_path:path}/images", response_model=ImageListResponse)
async def list_game_images(folder_path: str):
    """
    List all images in the game folder.

    Phase 24.0: Returns gallery of images for curator to select from.

    Args:
        folder_path: Path to game folder (URL encoded)

    Returns:
        ImageListResponse with list of images
    """
    try:
        config = get_config()
        library_root = config.library_roots[0]

        # Resolve full path
        full_path = _resolve_folder_path(folder_path, library_root)

        if not full_path.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Folder not found: {folder_path}"
            )

        # Image extensions
        image_extensions = {'.jpg', '.jpeg', '.png', '.webp', '.gif', '.bmp'}

        images = []

        # Scan for images
        for file_path in full_path.rglob('*'):
            if not file_path.is_file():
                continue

            if file_path.suffix.lower() not in image_extensions:
                continue

            # Determine image type
            image_type = 'screenshot'  # Default
            if file_path.parent.name.lower() in ['extras', 'art', 'artwork']:
                image_type = 'art'
            elif 'cover' in file_path.name.lower():
                image_type = 'cover'

            # Create URL (relative to game folder)
            relative_path = file_path.relative_to(full_path)

            images.append({
                "path": str(relative_path),
                "name": file_path.name,
                "type": image_type,
                "url": f"/api/images/{folder_path}/{relative_path}",  # Would need image serving endpoint
            })

        return ImageListResponse(
            folder_path=folder_path,
            images=images,
            total_count=len(images),
            message=f"Found {len(images)} images"
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error listing images: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error listing images: {str(e)}"
        )


# ============================================================================
# PHASE 19.6: Version Manager APIs (Continued from above)
# ============================================================================
async def delete_game_version(vndb_id: str, version_id: str):
    """
    Remove a version from this game (unlink).

    Args:
        vndb_id: VNDB ID
        version_id: Version ID (folder path) to remove

    Returns:
        DeleteVersionResponse with result
    """
    try:
        from ..metadata.manager import get_resource_manager

        config = get_config()
        library_root = config.library_roots[0]
        resource_manager = get_resource_manager(library_root, quota_gb=2.0)

        version_path = Path(version_id)

        # Load metadata for this version's folder
        metadata_dict = resource_manager.load_metadata(version_path)
        if not metadata_dict:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"No metadata found for version: {version_id}"
            )

        metadata = UnifiedMetadata(**metadata_dict)

        # Remove the version
        original_count = len(metadata.versions)
        metadata.versions = [v for v in metadata.versions if v.path != version_id]

        if len(metadata.versions) == original_count:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Version not found: {version_id}"
            )

        # If this was the last version, clear vndb_id
        if len(metadata.versions) == 0:
            metadata.vndb_id = None

        # Save metadata
        success = resource_manager.save_metadata(metadata.model_dump(), version_path)

        if success:
            logger.info(f"Deleted version {version_id} from {vndb_id}")
            return DeleteVersionResponse(
                success=True,
                vndb_id=vndb_id,
                deleted_version_id=version_id,
                remaining_count=len(metadata.versions),
                message=f"Removed version: {version_path.name}"
            )
        else:
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Failed to save metadata"
            )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error deleting version: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error deleting version: {str(e)}"
        )
