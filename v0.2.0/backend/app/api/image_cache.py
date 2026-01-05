"""
Image Cache Service

Phase 3: Local image caching for cover images.

This module provides:
- Download and cache cover images locally
- Serve cached images
- Cache management (cleanup, size limits)
"""

import logging
import hashlib
import aiohttp
from pathlib import Path
from typing import Optional, Dict
from urllib.parse import urlparse

logger = logging.getLogger(__name__)


class ImageCache:
    """
    Manages local image cache for cover images.
    """

    def __init__(self, cache_dir: Path, max_cache_size_mb: int = 500):
        """
        Initialize image cache.

        Args:
            cache_dir: Directory to store cached images
            max_cache_size_mb: Maximum cache size in MB (default: 500MB)
        """
        self.cache_dir = cache_dir
        self.max_cache_size = max_cache_size_mb * 1024 * 1024  # Convert to bytes
        self.cache_index: Dict[str, Path] = {}  # URL -> file path mapping

        # Create cache directory
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        self._load_cache_index()

    def _load_cache_index(self):
        """Load existing cache index from disk."""
        index_file = self.cache_dir / "cache_index.json"
        if index_file.exists():
            try:
                import json
                with open(index_file, 'r') as f:
                    self.cache_index = json.load(f)
                logger.info(f"Loaded {len(self.cache_index)} cached images")
            except Exception as e:
                logger.error(f"Failed to load cache index: {e}")
                self.cache_index = {}

    def _save_cache_index(self):
        """Save cache index to disk."""
        index_file = self.cache_dir / "cache_index.json"
        try:
            import json
            with open(index_file, 'w') as f:
                json.dump({str(k): str(v) for k, v in self.cache_index.items()}, f)
        except Exception as e:
            logger.error(f"Failed to save cache index: {e}")

    def _get_cache_key(self, url: str) -> str:
        """
        Generate cache key from URL.

        Uses MD5 hash of the URL as cache key.

        Args:
            url: Image URL

        Returns:
            MD5 hash as cache key
        """
        return hashlib.md5(url.encode()).hexdigest()

    def _get_file_extension(self, url: str) -> str:
        """
        Extract file extension from URL.

        Args:
            url: Image URL

        Returns:
            File extension (e.g., .jpg, .png)
        """
        parsed = urlparse(url)
        path = parsed.path
        ext = Path(path).suffix

        # Default to .jpg if no extension
        return ext if ext else ".jpg"

    def get_cached_path(self, url: str) -> Optional[Path]:
        """
        Get cached file path for a URL.

        Args:
            url: Image URL

        Returns:
            Path to cached file, or None if not cached
        """
        cache_key = self._get_cache_key(url)
        return self.cache_index.get(cache_key)

    def is_cached(self, url: str) -> bool:
        """
        Check if image is cached.

        Args:
            url: Image URL

        Returns:
            True if cached, False otherwise
        """
        return self.get_cached_path(url) is not None

    async def download_image(self, url: str) -> Optional[Path]:
        """
        Download and cache an image.

        Args:
            url: Image URL to download

        Returns:
            Path to cached file, or None if failed
        """
        cache_key = self._get_cache_key(url)

        # Check if already cached
        if cache_key in self.cache_index:
            cached_path = self.cache_index[cache_key]
            if cached_path.exists():
                logger.debug(f"Image already cached: {url}")
                return cached_path
            else:
                # Cache index says it exists, but file is missing
                del self.cache_index[cache_key]

        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(url, timeout=aiohttp.ClientTimeout(total=30)) as response:
                    if response.status == 200:
                        # Generate file path
                        ext = self._get_file_extension(url)
                        filename = f"{cache_key}{ext}"
                        file_path = self.cache_dir / filename

                        # Write image data
                        file_path.write_bytes(await response.read())

                        # Update cache index
                        self.cache_index[cache_key] = file_path
                        self._save_cache_index()

                        logger.info(f"Cached image: {url} -> {file_path}")

                        return file_path
                    else:
                        logger.warning(f"Failed to download image: HTTP {response.status}")
                        return None

        except Exception as e:
            logger.error(f"Error downloading image {url}: {e}")
            return None

    async def get_image(self, url: str) -> Optional[Path]:
        """
        Get image (cached or download).

        Args:
            url: Image URL

        Returns:
            Path to image file, or None if failed
        """
        # Try to get cached image first
        cached_path = self.get_cached_path(url)
        if cached_path and cached_path.exists():
            return cached_path

        # Not cached, download it
        return await self.download_image(url)

    def get_cache_size(self) -> int:
        """
        Get total cache size in bytes.

        Returns:
            Total cache size in bytes
        """
        total_size = 0
        for path in self.cache_index.values():
            if path.exists():
                total_size += path.stat().st_size
        return total_size

    def cleanup_cache(self, max_size_mb: Optional[int] = None):
        """
        Clean up cache if it exceeds size limit.

        Args:
            max_size_mb: Maximum cache size in MB (default: self.max_cache_size)
        """
        max_size = (max_size_mb * 1024 * 1024) if max_size_mb else self.max_cache_size
        current_size = self.get_cache_size()

        if current_size <= max_size:
            return

        # Cache is too big, clean up oldest files
        logger.warning(f"Cache size {current_size} > limit {max_size}, cleaning up...")

        # Sort files by modification time (oldest first)
        files_by_mtime = sorted(
            [(path, path.stat().st_mtime) for path in self.cache_index.values() if path.exists()],
            key=lambda x: x[1]
        )

        # Delete oldest files until size is under limit
        for path, mtime in files_by_mtime:
            if current_size <= max_size:
                break

            # Delete file
            try:
                file_size = path.stat().st_size
                path.unlink()
                current_size -= file_size

                # Remove from cache index
                cache_key = next((k for k, v in self.cache_index.items() if v == path), None)
                if cache_key:
                    del self.cache_index[cache_key]

                logger.info(f"Deleted cached file: {path}")
            except Exception as e:
                logger.error(f"Error deleting cached file {path}: {e}")

        self._save_cache_index()

    def clear_cache(self):
        """Clear all cached images."""
        for path in self.cache_index.values():
            if path.exists():
                try:
                    path.unlink()
                except Exception as e:
                    logger.error(f"Error deleting {path}: {e}")

        self.cache_index.clear()
        self._save_cache_index()
        logger.info("Cache cleared")


# Global image cache instance
_image_cache: ImageCache = None


def get_image_cache() -> ImageCache:
    """
    Get or create global image cache instance.

    Returns:
        ImageCache singleton
    """
    global _image_cache
    if _image_cache is None:
        from ..config import get_config
        config = get_config()
        cache_dir = config.config_dir / "cache" / "covers"

        # Max cache size: 500MB
        _image_cache = ImageCache(cache_dir, max_cache_size_mb=500)
        logger.info(f"Image cache initialized at: {cache_dir}")

    return _image_cache
