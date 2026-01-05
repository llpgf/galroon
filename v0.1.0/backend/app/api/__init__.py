"""
API endpoints for Galgame Library Manager.

Phase 3: Added v1 router support for modular routing.
"""

# Import v1 router (with /api/v1 prefix)
from .v1 import api_v1_router

# Import all existing routers for backward compatibility
from . import (
    analytics_router,
    backup_router,
    connectors_router,
    curator_router,
    games_router,
    history_router,
    organizer_router,
    scanner_router,
    scheduler_router,
    search_router,
    settings_router,
    system_router,
    trash_router,
    update_router,
    utilities_router,
)

__all__ = [
    "api_v1_router",
    "analytics_router",
    "backup_router",
    "connectors_router",
    "curator_router",
    "games_router",
    "history_router",
    "organizer_router",
    "scanner_router",
    "scheduler_router",
    "search_router",
    "settings_router",
    "system_router",
    "trash_router",
    "update_router",
    "utilities_router",
]
