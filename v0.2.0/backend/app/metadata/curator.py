"""
Curator Module for Galgame Library Manager.

**PHASE 10: The Curator Backend**

Provides manual correction and refinement tools for library management.

Key Features:
- Manual VNDB ID identification (force-link folder to ID)
- Immediate metadata fetching for identified games
- Version merging when ID already exists
- Field-level locking support

Usage:
    from app.metadata.curator import Curator

    curator = Curator(library_root, resource_manager)
    result = curator.identify_game(
        folder_path=Path("H:/Games/Unknown"),
        vndb_id="v12345"
    )
"""

import logging
from pathlib import Path
from typing import Optional, Dict, Any, List
from datetime import datetime

from .models import UnifiedMetadata, create_empty_metadata, GameVersion
from .manager import get_resource_manager
from .inventory import AssetDetector

logger = logging.getLogger(__name__)


class IdentificationResult:
    """
    Result of manual game identification.

    Attributes:
        success: Whether identification succeeded
        folder_path: Path to the game folder
        vndb_id: Assigned VNDB ID
        metadata: Fetched metadata
        merged: Whether this was merged into existing work
        message: Human-readable result message
    """
    def __init__(
        self,
        success: bool,
        folder_path: Path,
        vndb_id: str,
        metadata: Optional[UnifiedMetadata] = None,
        merged: bool = False,
        message: str = ""
    ):
        self.success = success
        self.folder_path = folder_path
        self.vndb_id = vndb_id
        self.metadata = metadata
        self.merged = merged
        self.message = message

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "success": self.success,
            "folder_path": str(self.folder_path),
            "vndb_id": self.vndb_id,
            "metadata": self.metadata.model_dump() if self.metadata else None,
            "merged": self.merged,
            "message": self.message
        }


class Curator:
    """
    Curator for manual game management and correction.

    Provides tools for:
    - Manually identifying unknown games
    - Force-linking folders to VNDB IDs
    - Merging versions when ID already exists
    - Fetching immediate metadata updates
    """

    def __init__(self, library_root: Path, quota_gb: float = 2.0):
        """
        Initialize the Curator.

        Args:
            library_root: Root library directory
            quota_gb: Metadata storage quota in GB
        """
        self.library_root = Path(library_root)
        self.resource_manager = get_resource_manager(self.library_root, quota_gb)
        self.asset_detector = AssetDetector()

        logger.info(f"Curator initialized for library: {self.library_root}")

    def identify_game(
        self,
        folder_path: Path,
        vndb_id: str,
        provider: str = "vndb",
        fetch_metadata: bool = True
    ) -> IdentificationResult:
        """
        Manually identify a game by linking it to a VNDB ID.

        This will:
        1. Load existing metadata (if any)
        2. Fetch metadata from VNDB using the provided ID
        3. Detect assets in the folder
        4. Add/update the folder as a version
        5. Merge if the VNDB ID already exists in another folder

        Args:
            folder_path: Path to the game folder
            vndb_id: VNDB identifier (e.g., "v12345")
            provider: Metadata provider to use (default: "vndb")
            fetch_metadata: Whether to fetch fresh metadata (default: True)

        Returns:
            IdentificationResult with details
        """
        logger.info(f"Manual identification: {folder_path} -> {vndb_id}")

        folder_path = Path(folder_path)

        # Validate folder exists
        if not folder_path.exists():
            return IdentificationResult(
                success=False,
                folder_path=folder_path,
                vndb_id=vndb_id,
                message=f"Folder not found: {folder_path}"
            )

        # Load existing metadata
        existing_metadata = self._load_metadata(folder_path)

        # Fetch new metadata from VNDB
        if fetch_metadata:
            new_metadata = self._fetch_from_provider(
                vndb_id=vndb_id,
                provider=provider,
                folder_name=folder_path.name
            )

            if not new_metadata:
                return IdentificationResult(
                    success=False,
                    folder_path=folder_path,
                    vndb_id=vndb_id,
                    message=f"Failed to fetch metadata from {provider} for ID: {vndb_id}"
                )
        else:
            new_metadata = create_empty_metadata()
            new_metadata.vndb_id = vndb_id

        # Detect assets
        detection_result = self.asset_detector.detect_directory(folder_path)

        # Merge with existing metadata (respecting locks)
        if existing_metadata:
            # Merge into existing metadata
            merged_metadata = self._merge_with_locks(
                existing=existing_metadata,
                new=new_metadata,
                folder_path=folder_path,
                assets=detection_result.assets
            )

            # Check if this is a version merge (same VNDB ID)
            merged = existing_metadata.vndb_id == vndb_id
            message = f"Updated metadata for {vndb_id}" if merged else f"Merged into {vndb_id}"
        else:
            # No existing metadata, use new metadata as base
            merged_metadata = new_metadata
            merged_metadata.vndb_id = vndb_id
            merged_metadata.add_version(
                path=str(folder_path),
                label=detection_result.version_label,
                is_primary=True,
                assets=detection_result.assets
            )

            merged = False
            message = f"Identified as {vndb_id}"

        # Save metadata
        success = self._save_metadata(merged_metadata, folder_path)

        if success:
            logger.info(f"Manual identification successful: {folder_path} -> {vndb_id}")
            return IdentificationResult(
                success=True,
                folder_path=folder_path,
                vndb_id=vndb_id,
                metadata=merged_metadata,
                merged=merged,
                message=message
            )
        else:
            return IdentificationResult(
                success=False,
                folder_path=folder_path,
                vndb_id=vndb_id,
                metadata=merged_metadata,
                merged=merged,
                message="Failed to save metadata"
            )

    def _load_metadata(self, folder_path: Path) -> Optional[UnifiedMetadata]:
        """Load existing metadata from folder."""
        try:
            metadata_dict = self.resource_manager.load_metadata(folder_path)
            if metadata_dict:
                return UnifiedMetadata(**metadata_dict)
        except Exception as e:
            logger.warning(f"Could not load existing metadata: {e}")
        return None

    def _fetch_from_provider(
        self,
        vndb_id: str,
        provider: str,
        folder_name: str
    ) -> Optional[UnifiedMetadata]:
        """Fetch metadata from provider using VNDB ID."""
        try:
            from .providers import get_vndb_provider

            if provider == "vndb":
                vndb_provider = get_vndb_provider()
                metadata = vndb_provider.fetch_by_id(vndb_id)

                if metadata:
                    logger.info(f"Fetched metadata for {vndb_id} from {provider}")
                    return metadata

        except Exception as e:
            logger.error(f"Error fetching from {provider}: {e}")

        return None

    def _merge_with_locks(
        self,
        existing: UnifiedMetadata,
        new: UnifiedMetadata,
        folder_path: Path,
        assets: List[str]
    ) -> UnifiedMetadata:
        """
        Merge new metadata into existing, respecting field locks.

        Args:
            existing: Existing metadata with locks
            new: New metadata to merge
            folder_path: Folder path for version
            assets: Detected assets

        Returns:
            Merged metadata
        """
        # Import merger
        from .merger import merge_metadata

        # Merge metadata (respecting locks)
        merged_dict, changes = merge_metadata(
            existing_metadata=existing.model_dump(),
            new_metadata=new.model_dump(),
            source="manual",
            prefer_traditional=True
        )

        merged = UnifiedMetadata(**merged_dict)

        # Add version
        merged.add_version(
            path=str(folder_path),
            label=f"{assets[0] if assets else 'Unknown'} Version",
            is_primary=False,  # Don't change primary if merging
            assets=assets
        )

        return merged

    def _save_metadata(self, metadata: UnifiedMetadata, folder_path: Path) -> bool:
        """Save metadata to folder."""
        try:
            metadata_dict = metadata.model_dump()
            success = self.resource_manager.save_metadata(metadata_dict, folder_path)

            if success:
                # Download cover image if available
                if metadata.cover_url and metadata.cover_url.value:
                    try:
                        self.resource_manager.download_metadata_image(
                            metadata_dict,
                            folder_path,
                            "cover"
                        )
                    except Exception as e:
                        logger.warning(f"Could not download cover: {e}")

            return success

        except Exception as e:
            logger.error(f"Error saving metadata: {e}")
            return False

    def find_work_by_vndb_id(self, vndb_id: str) -> List[Path]:
        """
        Find all folders that have the given VNDB ID.

        Useful for seeing all versions of a work.

        Args:
            vndb_id: VNDB identifier to search for

        Returns:
            List of folder paths with this VNDB ID
        """
        found_folders = []

        try:
            # Search all subdirectories
            for folder in self.library_root.rglob("*"):
                if not folder.is_dir():
                    continue

                # Load metadata
                metadata = self._load_metadata(folder)
                if metadata and metadata.vndb_id == vndb_id:
                    found_folders.append(folder)

        except Exception as e:
            logger.error(f"Error searching for VNDB ID {vndb_id}: {e}")

        return found_folders

    def merge_versions(
        self,
        vndb_id: str,
        primary_folder: Optional[Path] = None
    ) -> Dict[str, Any]:
        """
        Merge all versions of a work under one VNDB ID.

        Ensures all folders with the same VNDB ID have consistent metadata
        and are properly listed as versions.

        Args:
            vndb_id: VNDB identifier
            primary_folder: Optional folder to set as primary version

        Returns:
            Summary of merge operation
        """
        logger.info(f"Merging versions for {vndb_id}")

        folders = self.find_work_by_vndb_id(vndb_id)

        if not folders:
            return {
                "success": False,
                "vndb_id": vndb_id,
                "message": "No folders found with this VNDB ID"
            }

        # Load all metadata
        versions_metadata = []
        for folder in folders:
            metadata = self._load_metadata(folder)
            if metadata:
                versions_metadata.append((folder, metadata))

        # Use the most complete metadata as base
        base_metadata = max(
            versions_metadata,
            key=lambda x: len(x[1].model_dump(exclude_unset=True)),
            default=(None, create_empty_metadata())
        )[1]

        # Update all folders with consistent metadata
        updated_count = 0
        for folder, metadata in versions_metadata:
            # Skip if it's already the base
            if metadata == base_metadata:
                continue

            # Merge with base metadata
            merged = self._merge_with_locks(
                existing=metadata,
                new=base_metadata,
                folder_path=folder,
                assets=metadata.assets_detected or []
            )

            # Save
            if self._save_metadata(merged, folder):
                updated_count += 1

        return {
            "success": True,
            "vndb_id": vndb_id,
            "folders_found": len(folders),
            "folders_updated": updated_count,
            "message": f"Updated {updated_count} of {len(folders)} folders"
        }


# Global singleton
_curator: Optional[Curator] = None


def get_curator(library_root: Optional[Path] = None) -> Curator:
    """
    Get the global Curator singleton.

    Args:
        library_root: Library root path (required for first call)

    Returns:
        Curator instance
    """
    global _curator

    if _curator is None:
        if library_root is None:
            raise ValueError("library_root required for first Curator initialization")
        _curator = Curator(library_root)

    return _curator
