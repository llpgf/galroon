"""
Auth API - Sprint 10
Handles Google Drive OAuth flow.
"""

from fastapi import APIRouter, HTTPException, Request
from fastapi.responses import RedirectResponse
import logging
from app.connectors.gdrive import GoogleDriveService
from app.core.config import settings

logger = logging.getLogger(__name__)
router = APIRouter(prefix="/auth", tags=["auth"])

# Configuration from centralized settings
REDIRECT_URI = settings.GDRIVE_REDIRECT_URI
FRONTEND_SETTINGS_URL = settings.FRONTEND_SETTINGS_URL

@router.get("/gdrive/login")
async def gdrive_login():
    """Initiate Google Drive OAuth flow."""
    try:
        service = GoogleDriveService()
        auth_url = service.get_auth_url(redirect_uri=REDIRECT_URI)
        return {"auth_url": auth_url}
    except FileNotFoundError:
        raise HTTPException(status_code=404, detail="Client secrets not found. Please configure Google Cloud credentials.")
    except Exception as e:
        logger.error(f"Auth init failed: {e}")
        raise HTTPException(status_code=500, detail=str(e))

@router.get("/gdrive/callback")
async def gdrive_callback(code: str, error: str = None):
    """Handle Google Drive OAuth callback."""
    if error:
        logger.error(f"Auth callback error: {error}")
        return RedirectResponse(url=f"{FRONTEND_SETTINGS_URL}?auth_error={error}")
    
    try:
        service = GoogleDriveService()
        service.fetch_token(code=code, redirect_uri=REDIRECT_URI)
        return RedirectResponse(url=f"{FRONTEND_SETTINGS_URL}?auth_success=true")
    except Exception as e:
        logger.error(f"Auth token fetch failed: {e}")
        return RedirectResponse(url=f"{FRONTEND_SETTINGS_URL}?auth_error=token_exchange_failed")

@router.get("/gdrive/status")
async def gdrive_status():
    """Check if authenticated."""
    service = GoogleDriveService()
    return {"authenticated": service.is_authenticated()}
