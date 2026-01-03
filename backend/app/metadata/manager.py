"""
Resource manager for metadata and images.

Implements LocalFirst strategy (check for existing images) and Quota management.
"""

import os
import logging
import requests
from pathlib import Path
from typing import Optional, List, Dict, Any
from urllib.parse import urlparse

try:
    from PIL import Image
    from io import BytesIO
    PILLOW_AVAILABLE = True
except ImportError:
    PILLOW_AVAILABLE = False
    logging.warning("Pillow not available. Image validation will be disabled.")

from .normalizer import TextNormalizer

logger = logging.getLogger(__name__)


class ResourceManager:
    """
    Manages metadata and image resources.

    Features:
    - LocalFirst: Check for existing images before downloading
    - Quota: Track and enforce storage limits
    - Image validation: Check if images are valid
    - Download retries: Handle network errors
    """

    DEFAULT_MAX_METADATA_SIZE_GB = 2.0
    COMMON_IMAGE_NAMES = ["folder.jpg", "folder.png", "cover.jpg", "cover.png"]

    def __init__(
        self,
        library_root: Path,
        quota_gb: float = DEFAULT_MAX_METADATA_SIZE_GB
    ):
        """
        Initialize resource manager.

        Args:
            library_root: Root directory of game library
            quota_gb: Maximum storage for metadata in GB
        """
        self.library_root = library_root
        self.quota_bytes = quota_gb * 1024 * 1024 * 1024
        self.current_usage_bytes = 0

    def check_existing_cover(self, game_dir: Path) -> Optional[Path]:
        """
        Check for existing cover image in game directory (LocalFirst).

        Args:
            game_dir: Game directory path

        Returns:
            Path to existing cover or None
        """
        if not game_dir.exists():
            return None

        # Check for common image names
        for image_name in self.COMMON_IMAGE_NAMES:
            image_path = game_dir / image_name
            if image_path.exists() and image_path.is_file():
                logger.info(f"Found existing cover: {image_path}")
                return image_path

        # Check for any image file
        for ext in ["*.jpg", "*.jpeg", "*.png", "*.webp"]:
            matches = list(game_dir.glob(ext))
            if matches:
                # Return the first match
                return matches[0]

        return None

    def download_image(
        self,
        url: str,
        dest_path: Path,
        validate: bool = True
    ) -> bool:
        """
        Download image from URL.

        Args:
            url: Image URL
            dest_path: Destination path
            validate: If True, validate image after download

        Returns:
            True if successful
        """
        try:
            # Check quota first
            if not self.check_quota(estimate_size=5 * 1024 * 1024):  # Estimate 5MB
                logger.error(f"Quota exceeded, cannot download: {url}")
                return False

            # Download
            response = requests.get(url, timeout=30, stream=True)
            response.raise_for_status()

            # Ensure parent directory exists
            dest_path.parent.mkdir(parents=True, exist_ok=True)

            # Save to temp file first
            temp_path = dest_path.with_suffix(dest_path.suffix + ".tmp")
            with open(temp_path, 'wb') as f:
                for chunk in response.iter_content(chunk_size=8192):
                    if chunk:
                        f.write(chunk)

            # Validate if requested
            if validate and PILLOW_AVAILABLE:
                try:
                    img = Image.open(temp_path)
                    img.verify()
                    # Re-open for actual use (verify closes the file)
                    with Image.open(temp_path) as img:
                        img.load()
                except Exception as e:
                    logger.error(f"Image validation failed: {e}")
                    temp_path.unlink(missing_ok=True)
                    return False

            # Rename to final path
            temp_path.rename(dest_path)

            # Update quota
            file_size = dest_path.stat().st_size
            self.current_usage_bytes += file_size

            logger.info(f"Downloaded image: {dest_path} ({file_size} bytes)")
            return True

        except requests.exceptions.RequestException as e:
            logger.error(f"Failed to download image from {url}: {e}")
            return False
        except Exception as e:
            logger.error(f"Unexpected error downloading image: {e}")
            return False

    def download_metadata_image(
        self,
        metadata: Dict[str, Any],
        game_dir: Path,
        image_type: str = "cover"
    ) -> Optional[Path]:
        """
        Download image for a game (cover or screenshot).

        Args:
            metadata: Metadata dict with cover_url or screenshot_urls
            game_dir: Game directory
            image_type: "cover" or "screenshot"

        Returns:
            Path to downloaded image or None
        """
        if image_type == "cover":
            url = metadata.get("cover_url", "")
            if isinstance(url, dict):
                url = url.get("value", url)
            if not url:
                return None

            # Check for existing cover
            existing = self.check_existing_cover(game_dir)
            if existing:
                return existing

            # Download to folder.jpg
            dest_path = game_dir / "folder.jpg"

        elif image_type == "screenshots":
            urls = metadata.get("screenshot_urls", [])
            if isinstance(urls, dict):
                urls = urls.get("value", urls)
            if not urls or len(urls) == 0:
                return None

            # Download first screenshot
            url = urls[0]

            # Create screenshots directory
            screenshots_dir = game_dir / "screenshots"
            dest_path = screenshots_dir / f"screenshot1.jpg"

        else:
            logger.error(f"Unknown image type: {image_type}")
            return None

        # Download
        success = self.download_image(url, dest_path)
        if success:
            return dest_path
        return None

    def check_quota(self, estimate_size: int = 0) -> bool:
        """
        Check if adding content would exceed quota.

        Args:
            estimate_size: Estimated size of new content in bytes

        Returns:
            True if within quota
        """
        new_total = self.current_usage_bytes + estimate_size
        return new_total <= self.quota_bytes

    def get_quota_status(self) -> Dict[str, Any]:
        """
        Get current quota status.

        Returns:
            Dict with usage stats
        """
        usage_gb = self.current_usage_bytes / (1024 * 1024 * 1024)
        quota_gb = self.quota_bytes / (1024 * 1024 * 1024)
        remaining_bytes = self.quota_bytes - self.current_usage_bytes
        remaining_gb = remaining_bytes / (1024 * 1024 * 1024)

        return {
            "current_usage_bytes": self.current_usage_bytes,
            "current_usage_gb": round(usage_gb, 2),
            "quota_bytes": self.quota_bytes,
            "quota_gb": round(quota_gb, 2),
            "remaining_bytes": remaining_bytes,
            "remaining_gb": round(remaining_gb, 2),
            "usage_percent": round((usage_gb / quota_gb) * 100, 1) if quota_gb > 0 else 0
        }

    def calculate_usage(self) -> int:
        """
        Calculate current metadata storage usage.

        Scans library for metadata.json and image files.

        Returns:
            Total usage in bytes
        """
        total_size = 0

        if not self.library_root.exists():
            return 0

        # Count metadata.json files
        for metadata_file in self.library_root.rglob("metadata.json"):
            total_size += metadata_file.stat().st_size

        # Count folder images
        for image_name in self.COMMON_IMAGE_NAMES:
            for image_file in self.library_root.rglob(image_name):
                if image_file.is_file():
                    total_size += image_file.stat().st_size

        self.current_usage_bytes = total_size
        return total_size

    def save_metadata(
        self,
        metadata: Dict[str, Any],
        game_dir: Path,
        filename: str = "metadata.json"
    ) -> bool:
        """
        Save metadata to JSON file.

        Args:
            metadata: Metadata dict
            game_dir: Game directory
            filename: Metadata filename

        Returns:
            True if successful
        """
        try:
            import json

            # Ensure directory exists
            game_dir.mkdir(parents=True, exist_ok=True)

            # Save to file
            metadata_path = game_dir / filename
            with open(metadata_path, 'w', encoding='utf-8') as f:
                json.dump(metadata, f, indent=2, ensure_ascii=False)

            # Update quota
            file_size = metadata_path.stat().st_size
            self.current_usage_bytes += file_size

            logger.info(f"Saved metadata: {metadata_path}")
            return True

        except Exception as e:
            logger.error(f"Failed to save metadata: {e}")
            return False

    def load_metadata(self, game_dir: Path, filename: str = "metadata.json") -> Optional[Dict]:
        """
        Load metadata from JSON file.

        Args:
            game_dir: Game directory
            filename: Metadata filename

        Returns:
            Metadata dict or None
        """
        try:
            import json

            metadata_path = game_dir / filename
            if not metadata_path.exists():
                return None

            with open(metadata_path, 'r', encoding='utf-8') as f:
                data = json.load(f)

            logger.debug(f"Loaded metadata: {metadata_path}")
            return data

        except Exception as e:
            logger.error(f"Failed to load metadata: {e}")
            return None

    def cleanup_temp_files(self, game_dir: Path):
        """
        Clean up temporary download files.

        Args:
            game_dir: Game directory
        """
        # Remove .tmp files
        for temp_file in game_dir.rglob("*.tmp"):
            try:
                temp_file.unlink()
                logger.debug(f"Cleaned up temp file: {temp_file}")
            except Exception as e:
                logger.warning(f"Failed to cleanup temp file {temp_file}: {e}")


# Singleton instance
_resource_manager: Optional[ResourceManager] = None


def get_resource_manager(
    library_root: Path,
    quota_gb: float = 2.0
) -> ResourceManager:
    """
    Get or create resource manager singleton.

    Args:
        library_root: Library root directory
        quota_gb: Quota in GB

    Returns:
        ResourceManager instance
    """
    global _resource_manager
    if _resource_manager is None:
        _resource_manager = ResourceManager(library_root, quota_gb)
    return _resource_manager
