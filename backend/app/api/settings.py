"""
Settings API endpoints for Galgame Library Manager.

**PHASE 19.6: Library Roots Management**
**PHASE 19.8: Connectors Management**

Provides REST API endpoints for managing:
- Library roots
- Connector status (VNDB, Bangumi, Steam)
"""

import logging
import json
from pathlib import Path
from typing import List, Optional, Dict, Any

from fastapi import APIRouter, HTTPException, status
from pydantic import BaseModel, Field

from ..config import get_config, Config

logger = logging.getLogger(__name__)

# Create router
router = APIRouter(prefix="/api/settings", tags=["settings"])


# ============================================================================
# Pydantic Models
# ============================================================================

class LibraryRoot(BaseModel):
    """Represents a library root path."""
    id: str = Field(..., description="Unique ID for this root")
    path: str = Field(..., description="Full path to library root")
    exists: bool = Field(..., description="Whether path exists on disk")


class LibraryRootsResponse(BaseModel):
    """Response model for library roots list."""
    roots: List[LibraryRoot]
    total_count: int


class AddRootRequest(BaseModel):
    """Request model for adding a library root."""
    path: str = Field(..., description="Path to add as library root")


class AddRootResponse(BaseModel):
    """Response model for adding a root."""
    success: bool
    root: LibraryRoot
    message: str


class DeleteRootResponse(BaseModel):
    """Response model for deleting a root."""
    success: bool
    deleted_root_id: str
    remaining_count: int
    message: str


class SetPrimaryRootResponse(BaseModel):
    """Response model for setting primary root."""
    success: bool
    primary_root_id: str
    primary_root_path: str
    message: str


class ConnectorInfo(BaseModel):
    """Information about a connector."""
    name: str = Field(..., description="Connector name (vndb, bangumi, steam)")
    display_name: str = Field(..., description="Human-readable name")
    enabled: bool = Field(..., description="Whether connector is enabled")
    available: bool = Field(..., description="Whether connector is available")
    description: str = Field(..., description="Connector description")


class ConnectorsResponse(BaseModel):
    """Response model for connectors list."""
    connectors: List[ConnectorInfo]


class UpdateConnectorRequest(BaseModel):
    """Request model for updating connector status."""
    enabled: bool = Field(..., description="New enabled status")


class UpdateConnectorResponse(BaseModel):
    """Response model for updating connector."""
    success: bool
    connector: ConnectorInfo
    message: str


# ============================================================================
# API Endpoints
# ============================================================================

@router.get("/roots", response_model=LibraryRootsResponse)
async def get_library_roots():
    """
    Get all configured library roots.

    Returns:
        LibraryRootsResponse with list of all roots
    """
    try:
        config = get_config()

        # Convert library_roots to LibraryRoot objects
        roots = []
        for i, root_path in enumerate(config.library_roots):
            roots.append(LibraryRoot(
                id=f"root_{i}",
                path=str(root_path),
                exists=root_path.exists()
            ))

        return LibraryRootsResponse(
            roots=roots,
            total_count=len(roots)
        )

    except Exception as e:
        logger.error(f"Error getting library roots: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting library roots: {str(e)}"
        )


@router.post("/roots", response_model=AddRootResponse)
async def add_library_root(request: AddRootRequest):
    """
    Add a new library root to the configuration.

    Args:
        request: AddRootRequest with path to add

    Returns:
        AddRootResponse with result
    """
    try:
        config = get_config()
        new_root = Path(request.path)

        # Validate path exists
        if not new_root.exists():
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Path does not exist: {new_root}"
            )

        # Validate path is absolute to avoid ambiguous root handling
        if not new_root.is_absolute():
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Path must be absolute"
            )

        # Check if already in list
        if any(str(root) == str(new_root) for root in config.library_roots):
            # Return existing root
            for i, root in enumerate(config.library_roots):
                if str(root) == str(new_root):
                    return AddRootResponse(
                        success=True,
                        root=LibraryRoot(
                            id=f"root_{i}",
                            path=str(root),
                            exists=True
                        ),
                        message="Root already exists"
                    )

        # Add to list (would need to persist this in real implementation)
        config.library_roots.append(new_root)

        # Generate ID
        root_id = f"root_{len(config.library_roots) - 1}"

        logger.info(f"Added library root: {new_root}")

        return AddRootResponse(
            success=True,
            root=LibraryRoot(
                id=root_id,
                path=str(new_root),
                exists=True
            ),
            message=f"Added library root: {new_root.name}"
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error adding library root: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error adding library root: {str(e)}"
        )


@router.delete("/roots/{root_id}", response_model=DeleteRootResponse)
async def delete_library_root(root_id: str):
    """
    Remove a library root from the configuration.

    Args:
        root_id: Root ID to remove (e.g., "root_0")

    Returns:
        DeleteRootResponse with result
    """
    try:
        config = get_config()

        # Parse root index from ID
        if not root_id.startswith("root_"):
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Invalid root ID format"
            )

        try:
            root_index = int(root_id.split("_")[1])
        except (ValueError, IndexError):
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Invalid root ID format"
            )

        # Check if index is valid
        if root_index < 0 or root_index >= len(config.library_roots):
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Root not found: {root_id}"
            )

        # Don't allow deleting the last root
        if len(config.library_roots) == 1:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Cannot delete the last library root"
            )

        # Remove from list
        deleted_root = config.library_roots.pop(root_index)

        logger.info(f"Deleted library root: {deleted_root}")

        return DeleteRootResponse(
            success=True,
            deleted_root_id=root_id,
            remaining_count=len(config.library_roots),
            message=f"Removed library root: {deleted_root.name}"
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error deleting library root: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error deleting library root: {str(e)}"
        )


@router.patch("/roots/{root_id}/set_primary", response_model=SetPrimaryRootResponse)
async def set_primary_root(root_id: str):
    """
    Set a library root as the primary root.

    This moves the specified root to the first position in the library_roots list,
    making it the primary root used by the application.

    Args:
        root_id: Root ID to set as primary (e.g., "root_0")

    Returns:
        SetPrimaryRootResponse with result

    Example:
        PATCH /api/settings/roots/root_1/set_primary
    """
    try:
        config = get_config()

        # Parse root index from ID
        if not root_id.startswith("root_"):
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Invalid root ID format"
            )

        try:
            root_index = int(root_id.split("_")[1])
        except (ValueError, IndexError):
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail="Invalid root ID format"
            )

        # Check if index is valid
        if root_index < 0 or root_index >= len(config.library_roots):
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"Root not found: {root_id}"
            )

        # If already first, nothing to do
        if root_index == 0:
            return SetPrimaryRootResponse(
                success=True,
                primary_root_id=root_id,
                primary_root_path=str(config.library_roots[0]),
                message="Root is already the primary root"
            )

        # Move root to first position
        primary_root = config.library_roots.pop(root_index)
        config.library_roots.insert(0, primary_root)

        # Update config.library_root to point to new primary
        config.library_root = config.library_roots[0]

        logger.info(f"Set primary root: {primary_root}")

        return SetPrimaryRootResponse(
            success=True,
            primary_root_id=root_id,
            primary_root_path=str(primary_root),
            message=f"Set primary root: {primary_root.name}"
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error setting primary root: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error setting primary root: {str(e)}"
        )


# ============================================================================
# PHASE 19.8: Connectors Management
# ============================================================================

def _get_connectors_config_path() -> Path:
    """Get path to connectors configuration file."""
    config = get_config()
    return config.config_dir / "connectors.json"


def _load_connectors_config() -> Dict[str, bool]:
    """Load connectors enabled status from config file."""
    config_path = _get_connectors_config_path()

    # Default: all connectors enabled
    default_config = {
        "vndb": True,
        "bangumi": True,
        "steam": False  # Steam disabled by default
    }

    if not config_path.exists():
        # Create default config file
        try:
            with open(config_path, 'w', encoding='utf-8') as f:
                json.dump(default_config, f, indent=2)
            logger.info(f"Created default connectors config: {config_path}")
        except Exception as e:
            logger.error(f"Failed to create connectors config: {e}")

        return default_config

    try:
        with open(config_path, 'r', encoding='utf-8') as f:
            return json.load(f)
    except Exception as e:
        logger.error(f"Failed to load connectors config: {e}")
        return default_config


def _save_connectors_config(config: Dict[str, bool]) -> bool:
    """Save connectors enabled status to config file."""
    config_path = _get_connectors_config_path()

    try:
        with open(config_path, 'w', encoding='utf-8') as f:
            json.dump(config, f, indent=2)
        return True
    except Exception as e:
        logger.error(f"Failed to save connectors config: {e}")
        return False


@router.get("/connectors", response_model=ConnectorsResponse)
async def get_connectors():
    """
    Get all connectors and their status.

    Returns:
        ConnectorsResponse with list of all connectors

    Example:
        GET /api/settings/connectors
    """
    try:
        # Load connector enabled status
        enabled_config = _load_connectors_config()

        # Define all available connectors
        connectors = [
            ConnectorInfo(
                name="vndb",
                display_name="VNDB",
                enabled=enabled_config.get("vndb", True),
                available=True,
                description="Visual Novel Database - Primary metadata source"
            ),
            ConnectorInfo(
                name="bangumi",
                display_name="Bangumi",
                enabled=enabled_config.get("bangumi", True),
                available=True,
                description="Chinese ACG database - Chinese metadata"
            ),
            ConnectorInfo(
                name="steam",
                display_name="Steam",
                enabled=enabled_config.get("steam", False),
                available=True,
                description="Steam - Game detection and assets"
            )
        ]

        return ConnectorsResponse(connectors=connectors)

    except Exception as e:
        logger.error(f"Error getting connectors: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting connectors: {str(e)}"
        )


@router.patch("/connectors/{connector_name}", response_model=UpdateConnectorResponse)
async def update_connector(connector_name: str, request: UpdateConnectorRequest):
    """
    Update connector enabled status.

    Args:
        connector_name: Connector name (vndb, bangumi, steam)
        request: Update request with enabled status

    Returns:
        UpdateConnectorResponse with result

    Example:
        PATCH /api/settings/connectors/vndb
        { "enabled": false }
    """
    try:
        # Validate connector name
        valid_connectors = ["vndb", "bangumi", "steam"]
        if connector_name not in valid_connectors:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"Invalid connector: {connector_name}. Must be one of: {', '.join(valid_connectors)}"
            )

        # Load current config
        enabled_config = _load_connectors_config()

        # Update enabled status
        enabled_config[connector_name] = request.enabled

        # Save config
        if not _save_connectors_config(enabled_config):
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="Failed to save connector configuration"
            )

        # Get display name
        display_names = {
            "vndb": "VNDB",
            "bangumi": "Bangumi",
            "steam": "Steam"
        }

        descriptions = {
            "vndb": "Visual Novel Database - Primary metadata source",
            "bangumi": "Chinese ACG database - Chinese metadata",
            "steam": "Steam - Game detection and assets"
        }

        logger.info(f"Updated connector {connector_name}: enabled={request.enabled}")

        return UpdateConnectorResponse(
            success=True,
            connector=ConnectorInfo(
                name=connector_name,
                display_name=display_names[connector_name],
                enabled=request.enabled,
                available=True,
                description=descriptions[connector_name]
            ),
            message=f"Connector '{display_names[connector_name]}' {'enabled' if request.enabled else 'disabled'}"
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error updating connector {connector_name}: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error updating connector: {str(e)}"
        )


# ============================================================================
# PHASE 19.5: Scanner Configuration
# ============================================================================

class ScannerConfigResponse(BaseModel):
    """Response model for scanner configuration."""
    scan_on_startup: bool
    scan_interval_min: int


class UpdateScannerConfigRequest(BaseModel):
    """Request model for updating scanner configuration."""
    scan_on_startup: Optional[bool] = Field(None, description="Enable/disable scan on startup")
    scan_interval_min: Optional[int] = Field(None, description="Scan interval in minutes (0 = manual)")


@router.get("/scanner", response_model=ScannerConfigResponse)
async def get_scanner_config():
    """
    Get scanner configuration.

    Phase 19.5: Returns scanner settings from config

    Returns:
        ScannerConfigResponse with current settings

    Example:
        GET /api/settings/scanner
    """
    try:
        config = get_config()

        return ScannerConfigResponse(
            scan_on_startup=config.scan_on_startup,
            scan_interval_min=config.scan_interval_min
        )

    except Exception as e:
        logger.error(f"Error getting scanner config: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error getting scanner config: {str(e)}"
        )


@router.post("/scanner", response_model=dict)
async def update_scanner_config(request: UpdateScannerConfigRequest):
    """
    Update scanner configuration.

    Phase 19.5: Persists scanner settings to settings.json

    Args:
        request: UpdateScannerConfigRequest with new settings

    Returns:
        Success message

    Example:
        POST /api/settings/scanner
        { "scan_on_startup": true, "scan_interval_min": 60 }
    """
    try:
        config = get_config()

        # Validate scan_interval_min
        if request.scan_interval_min is not None:
            if request.scan_interval_min < 0:
                raise HTTPException(
                    status_code=status.HTTP_400_BAD_REQUEST,
                    detail="scan_interval_min must be >= 0 (0 = manual mode)"
                )

        # Update config
        config.update_scanner_settings(
            scan_on_startup=request.scan_on_startup,
            scan_interval_min=request.scan_interval_min
        )

        logger.info(f"Updated scanner config: scan_on_startup={request.scan_on_startup}, scan_interval_min={request.scan_interval_min}")

        return {
            "success": True,
            "message": "Scanner configuration updated",
            "scanner": {
                "scan_on_startup": config.scan_on_startup,
                "scan_interval_min": config.scan_interval_min
            }
        }

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error updating scanner config: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error updating scanner config: {str(e)}"
        )

# ============================================================================
# PHASE 24.5: Settings Import/Export
# ============================================================================

class SettingsExport(BaseModel):
    """Model for full settings export."""
    version: int = 1
    timestamp: str
    library_roots: List[str]
    scanner: Dict[str, Any]
    connectors: Dict[str, bool]
    update: Dict[str, Any]


class ImportSettingsResponse(BaseModel):
    """Response for settings import."""
    success: bool
    message: str
    changes: List[str]


@router.get("/export", response_model=SettingsExport)
async def export_settings():
    """
    Export all application settings.
    
    Returns:
        JSON structure containing all configuration
    """
    try:
        config = get_config()
        
        # Load connector config
        connectors_config = _load_connectors_config()
        
        return SettingsExport(
            version=1,
            timestamp=datetime.now().isoformat(),
            library_roots=[str(r) for r in config.library_roots],
            scanner={
                "scan_on_startup": config.scan_on_startup,
                "scan_interval_min": config.scan_interval_min
            },
            connectors=connectors_config,
            update={
                "auto_check_enabled": config.auto_check_enabled,
                "check_interval_hours": config.check_interval_hours
            }
        )
    except Exception as e:
        logger.error(f"Error exporting settings: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error exporting settings: {str(e)}"
        )


@router.post("/import", response_model=ImportSettingsResponse)
async def import_settings(settings: SettingsExport):
    """
    Import application settings.
    
    Args:
        settings: SettingsExport model
    
    Returns:
        Result of import with list of changes
    """
    try:
        config = get_config()
        changes = []
        
        # Update Library Roots
        # We append new roots, but don't delete existing ones to be safe?
        # Or we replace? "Import" usually implies replacement or merge.
        # Let's merge: add if missing.
        for path_str in settings.library_roots:
            path = Path(path_str)
            if path not in config.library_roots:
                # Basic validation
                if path.exists():
                     config.library_roots.append(path)
                     changes.append(f"Added library root: {path}")
                else:
                    logger.warning(f"Skipping non-existent root in import: {path}")

        # Update Scanner
        if settings.scanner:
            config.update_scanner_settings(
                scan_on_startup=settings.scanner.get("scan_on_startup", config.scan_on_startup),
                scan_interval_min=settings.scanner.get("scan_interval_min", config.scan_interval_min)
            )
            changes.append("Updated scanner configuration")

        # Update Connectors
        if settings.connectors:
            _save_connectors_config(settings.connectors)
            changes.append("Updated connectors configuration")

        # Update Update Settings
        if settings.update:
            config.update_update_settings(
                auto_check_enabled=settings.update.get("auto_check_enabled", config.auto_check_enabled),
                check_interval_hours=settings.update.get("check_interval_hours", config.check_interval_hours)
            )
            changes.append("Updated update configuration")
            
        return ImportSettingsResponse(
            success=True,
            message="Settings imported successfully",
            changes=changes
        )
        
    except Exception as e:
        logger.error(f"Error importing settings: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error importing settings: {str(e)}"
        )
