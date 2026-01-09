"""
Backup API - Sprint 10
Handles Cloud Backup operations.
"""

from fastapi import APIRouter, HTTPException, Depends
from typing import Dict, Any
import logging
import os
import zipfile
import tempfile
from datetime import datetime
from app.connectors.gdrive import GoogleDriveService
from app.core.database import get_database

logger = logging.getLogger(__name__)
router = APIRouter(prefix="/sync", tags=["backup"])


@router.get("/status")
async def get_sync_status() -> Dict[str, Any]:
    """Get cloud backup status."""
    service = GoogleDriveService()
    if not service.is_authenticated():
        return {
            "authenticated": False,
            "last_backup": None,
            "status": "unauthenticated"
        }
    
    try:
        backups = service.list_backups()
        last_backup = backups[0] if backups else None
        
        return {
            "authenticated": True,
            "last_backup": last_backup['modifiedTime'] if last_backup else None,
            "backup_file": last_backup['name'] if last_backup else None,
            "status": "idle"
        }
    except Exception as e:
        logger.error(f"Failed to get sync status: {e}")
        return {
            "authenticated": True,
            "error": str(e),
            "status": "error"
        }


@router.post("/gdrive")
async def trigger_gdrive_sync():
    """Trigger immediate backup to Google Drive."""
    service = GoogleDriveService()
    if not service.is_authenticated():
        raise HTTPException(status_code=401, detail="Google Drive not authenticated")

    db = get_database()
    db_path = db.db_path
    
    if not os.path.exists(db_path):
        raise HTTPException(status_code=404, detail="Database file not found")

    try:
        # Create temp zip
        with tempfile.NamedTemporaryFile(suffix='.zip', delete=False) as tmp_zip:
            zip_path = tmp_zip.name
            
        with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as zf:
            zf.write(db_path, arcname="library.db")
            # We could add config files here too
            
        # Upload
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"galroon_backup_{timestamp}.zip"
        
        # For simplicity, we might want a fixed name "galroon_backup_latest.zip" 
        # or keep history. Let's keep history but maybe user wants latest.
        # Requirement says "Auto sync... one click restore".
        # Let's use a fixed name for the latest backup to avoid clutter, 
        # or rely on GDrive versioning. 
        # Let's use "galroon_library_backup.zip" and let Drive handle versions.
        target_filename = "galroon_library_backup.zip"
        
        file_id = service.upload_file(zip_path, target_filename)
        
        # Cleanup
        os.unlink(zip_path)
        
        return {
            "success": True,
            "file_id": file_id,
            "timestamp": timestamp
        }
        
    except Exception as e:
        logger.error(f"Backup failed: {e}")
        if os.path.exists(zip_path):
            os.unlink(zip_path)
        raise HTTPException(status_code=500, detail=str(e))
