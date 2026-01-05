"""
Image Cache API Router

Phase 3: Image download and caching endpoints.

Provides endpoints for:
- Downloading and caching cover images
- Managing cache size
- Serving cached images
"""

import logging
from pathlib import Path
from typing import Optional
from fastapi import APIRouter, HTTPException, status, Response
from pydantic import BaseModel, Field

from ..image_cache import get_image_cache

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/images", tags=["Images"])


class ImageDownloadRequest(BaseModel):
    """Request to download and cache an image."""
    url: str = Field(..., description="Image URL to download")
    force: bool = Field(default=False, description="Force re-download even if cached")


class CacheInfoResponse(BaseModel):
    """Response with cache information."""
    total_size_bytes: int
    total_size_mb: float
    file_count: int
    cache_dir: str


@router.post("/download")
async def download_image(request: ImageDownloadRequest):
    """
    Download and cache a cover image.

    Args:
        request: Image download request

    Returns:
        File path of cached/downloaded image
    """
    cache = get_image_cache()

    if request.force and cache.is_cached(request.url):
        # Delete existing cache if force is set
        cached_path = cache.get_cached_path(request.url)
        if cached_path and cached_path.exists():
            cached_path.unlink()
            logger.info(f"Force deleted cached image: {request.url}")

    # Get or download image
    file_path = await cache.get_image(request.url)

    if file_path and file_path.exists():
        logger.info(f"Image ready: {request.url} -> {file_path}")
        return {
            "success": True,
            "path": str(file_path),
            "filename": file_path.name
        }
    else:
        logger.error(f"Failed to download image: {request.url}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Failed to download image"
        )


@router.get("/cache-info", response_model=CacheInfoResponse)
async def get_cache_info():
    """
    Get cache information.

    Returns:
        Cache size and file count
    """
    cache = get_image_cache()
    total_size_bytes = cache.get_cache_size()
    total_size_mb = total_size_bytes / (1024 * 1024)

    return CacheInfoResponse(
        total_size_bytes=total_size_bytes,
        total_size_mb=round(total_size_mb, 2),
        file_count=len(cache.cache_index),
        cache_dir=str(cache.cache_dir)
    )


@router.post("/cache/cleanup")
async def cleanup_cache(max_size_mb: Optional[int] = None):
    """
    Clean up cache if it exceeds size limit.

    Args:
        max_size_mb: Maximum cache size in MB (optional, uses default 500MB)

    Returns:
        Cleanup result
    """
    cache = get_image_cache()
    cache.cleanup_cache(max_size_mb)

    total_size_bytes = cache.get_cache_size()
    total_size_mb = total_size_bytes / (1024 * 1024)

    logger.info(f"Cache cleanup completed. New size: {total_size_mb:.2f}MB")

    return {
        "success": True,
        "total_size_mb": round(total_size_mb, 2),
        "file_count": len(cache.cache_index)
    }


@router.post("/cache/clear")
async def clear_cache():
    """
    Clear all cached images.

    Returns:
        Clear result
    """
    cache = get_image_cache()
    cache.clear_cache()

    logger.info("Cache cleared")

    return {
        "success": True,
        "message": "All cached images have been deleted"
    }


@router.get("/cached/{cache_key}")
async def serve_cached_image(cache_key: str):
    """
    Serve a cached image.

    Args:
        cache_key: MD5 hash of the original URL

    Returns:
        Image file as response
    """
    cache = get_image_cache()

    # Find cached image
    for url, path in cache.cache_index.items():
        file_key = cache._get_cache_key(url)
        if file_key == cache_key and path.exists():
            # Read and return image
            with open(path, 'rb') as f:
                content = f.read()

            # Determine content type
            ext = path.suffix.lower()
            content_type = {
                '.jpg': 'image/jpeg',
                '.jpeg': 'image/jpeg',
                '.png': 'image/png',
                '.webp': 'image/webp',
                '.gif': 'image/gif'
            }.get(ext, 'image/jpeg')

            logger.debug(f"Serving cached image: {path}")

            return Response(
                content=content,
                media_type=content_type,
                headers={
                    "Cache-Control": f"public, max-age={7*24*60*60}"  # 7 days
                }
            )

    raise HTTPException(
        status_code=status.HTTP_404_NOT_FOUND,
        detail=f"Image not found in cache: {cache_key}"
    )
