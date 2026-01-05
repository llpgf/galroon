"""
Update API Endpoints.

Provides endpoints for checking and managing application updates.
Phase 24.5: System Governance - Auto-update system
"""

import logging
import httpx
from typing import Optional, Dict, Any
from datetime import datetime
from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel
from packaging import version

from ..config import get_config

logger = logging.getLogger(__name__)


class UpdateCheckResponse(BaseModel):
    """Response model for update check."""
    has_update: bool
    current_version: str
    latest_version: str
    release_url: Optional[str]
    release_notes: Optional[str]
    published_at: Optional[str]


class UpdateConfigResponse(BaseModel):
    """Response model for update configuration."""
    auto_check_enabled: bool
    check_interval_hours: int
    last_check_at: Optional[str]


router = APIRouter(prefix="/api/settings/update", tags=["update"])

# Current version (should match package.json)
CURRENT_VERSION = "1.0.0"
GITHUB_REPO = "anthropics/claude-code"  # Example repo, replace with actual repo

# Store last check time (in-memory only)
_last_check_at: Optional[str] = None


@router.get("/check", response_model=UpdateCheckResponse)
async def check_for_updates():
    """
    Check for updates from GitHub releases.

    Returns:
        Update check result

    Example:
        GET /api/settings/update/check
    """
    try:
        async with httpx.AsyncClient() as client:
            # Fetch latest release from GitHub
            response = await client.get(
                f"https://api.github.com/repos/{GITHUB_REPO}/releases/latest",
                headers={
                    "Accept": "application/vnd.github.v3+json",
                    "User-Agent": "Galgame-Library-Manager"
                },
                timeout=10.0
            )

            if response.status_code != 200:
                logger.error(f"GitHub API error: {response.status_code}")
                raise HTTPException(
                    status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                    detail="Failed to check for updates"
                )

            release_data = response.json()
            latest_version = release_data["tag_name"].lstrip("v")
            release_url = release_data["html_url"]
            release_notes = release_data.get("body", "")
            published_at = release_data.get("published_at", "")

            # Compare versions using semantic versioning
            has_update = version.parse(latest_version) > version.parse(CURRENT_VERSION)

            # Update last check time
            global _last_check_at
            _last_check_at = datetime.now().isoformat()

            return UpdateCheckResponse(
                has_update=has_update,
                current_version=CURRENT_VERSION,
                latest_version=latest_version,
                release_url=release_url,
                release_notes=release_notes,
                published_at=published_at
            )

    except httpx.TimeoutException:
        logger.error("GitHub API timeout")
        raise HTTPException(
            status_code=status.HTTP_504_GATEWAY_TIMEOUT,
            detail="Update check timed out"
        )
    except Exception as e:
        logger.error(f"Failed to check for updates: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Failed to check for updates: {str(e)}"
        )


@router.get("/config", response_model=UpdateConfigResponse)
async def get_update_config():
    """
    Get update configuration.

    Phase 24.5: Loads from config file (settings.json)

    Returns:
        Current update configuration

    Example:
        GET /api/settings/update/config
    """
    config = get_config()

    return UpdateConfigResponse(
        auto_check_enabled=config.auto_check_enabled,
        check_interval_hours=config.check_interval_hours,
        last_check_at=_last_check_at
    )


class UpdateConfigRequest(BaseModel):
    """Request model for update configuration."""
    auto_check_enabled: bool
    check_interval_hours: int = 24


@router.post("/config", response_model=UpdateConfigResponse)
async def update_update_config(request: UpdateConfigRequest):
    """
    Update update configuration.

    Phase 24.5: Saves to config file (settings.json)

    Args:
        request: Update configuration

    Returns:
        Updated configuration

    Example:
        POST /api/settings/update/config
        {
            "auto_check_enabled": true,
            "check_interval_hours": 24
        }
    """
    config = get_config()

    # Update and persist to settings.json
    config.update_update_settings(
        auto_check_enabled=request.auto_check_enabled,
        check_interval_hours=request.check_interval_hours
    )

    return UpdateConfigResponse(
        auto_check_enabled=config.auto_check_enabled,
        check_interval_hours=config.check_interval_hours,
        last_check_at=_last_check_at
    )
