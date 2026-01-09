"""
API endpoints for Galgame Library Manager.

Phase 3: Added v1 router support for modular routing.

Import routers directly from modules to avoid circular dependencies.
"""

# Import v1 router (with /api/v1 prefix)
from .v1 import api_v1_router

# Import all existing routers directly
from .analytics import router as analytics_router
from .backup import router as backup_router
from .connectors import router as connectors_router
from .curator import router as curator_router
from .games import router as games_router
from .history import router as history_router
from .v1.image_cache_api import router as image_cache_api_router  # Router is in v1/, not image_cache service
from .organizer import router as organizer_router
from .scanner import router as scanner_router
from .scheduler import router as scheduler_router
from .search import router as search_router
from .settings import router as settings_router
from .system import router as system_router
from .trash import router as trash_router
from .update import router as update_router
from .utilities import router as utilities_router

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
