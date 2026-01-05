"""
API v1 Endpoints

This module aggregates all API v1 routes with a common /api/v1 prefix.

Phase 3: Added WebSocket support for real-time updates
Sprint 5: Added Canonical Layer & MatchCluster routers
"""

import json
import logging
from fastapi import APIRouter, WebSocket, WebSocketDisconnect
from fastapi.responses import HTMLResponse

# Import WebSocket manager (websocket.py is in app/api/, so ../websocket = app/api/websocket)
from ..websocket import get_ws_manager, ScanProgressUpdate

# Import routers from sibling modules in app/api/
# These are defined in app/api/*.py files
from ..analytics import router as analytics_router
from ..backup import router as backup_router
from ..connectors import router as connectors_router
from ..curator import router as curator_router
from ..games import router as games_router
from ..history import router as history_router
from .image_cache_api import router as image_cache_api_router  # Router is in v1/, not image_cache service
from ..organizer import router as organizer_router
from ..scanner import router as scanner_router
from ..scheduler import router as scheduler_router
from ..search import router as search_router
from ..settings import router as settings_router
from ..system import router as system_router
from ..trash import router as trash_router
from ..update import router as update_router
from ..utilities import router as utilities_router

# Sprint 5: Import new Canonical Layer routers from current v1 folder
from .decisions import router as decisions_router
from .library import router as library_router

logger = logging.getLogger(__name__)

# Create API v1 router
api_v1_router = APIRouter(prefix="/api/v1", tags=["API v1"])

# Include all routers under v1
api_v1_router.include_router(analytics_router)
api_v1_router.include_router(backup_router)
api_v1_router.include_router(connectors_router)
api_v1_router.include_router(curator_router)
api_v1_router.include_router(games_router)
api_v1_router.include_router(history_router)
api_v1_router.include_router(image_cache_api_router)
api_v1_router.include_router(organizer_router)
api_v1_router.include_router(scanner_router)
api_v1_router.include_router(scheduler_router)
api_v1_router.include_router(search_router)
api_v1_router.include_router(settings_router)
api_v1_router.include_router(system_router)
api_v1_router.include_router(trash_router)
api_v1_router.include_router(update_router)
api_v1_router.include_router(utilities_router)

# Sprint 5: Canonical Layer & MatchCluster routers
api_v1_router.include_router(decisions_router)
api_v1_router.include_router(library_router)


# ============================================================================
# PHASE 3: WebSocket Endpoints
# ============================================================================

@api_v1_router.websocket("/ws/scan-progress")
async def websocket_scan_progress(websocket: WebSocket):
    """
    WebSocket endpoint for real-time scan progress updates.

    Clients can connect to ws://127.0.0.1:{port}/api/v1/ws/scan-progress
    to receive real-time scan progress updates.

    Message format (client -> server):
        - subscribe: Subscribe to scan progress updates
        - ping: Keep-alive ping

    Message format (server -> client):
        - type: Message type ("scan_progress", "notification")
        - data: Message data (ScanProgressUpdate or notification)
    """
    ws_manager = get_ws_manager()

    try:
        await ws_manager.connect(websocket)
        logger.info("WebSocket client connected to scan progress")

        while True:
            data = await websocket.receive_text()

            if not data:
                continue

            try:
                message = json.loads(data)

                if message.get("action") == "ping":
                    # Respond to ping
                    await websocket.send_json({"action": "pong"})
                elif message.get("action") == "subscribe":
                    # Client wants to receive updates
                    await websocket.send_json({
                        "status": "subscribed",
                        "message": "Subscribed to scan progress updates"
                    })

            except json.JSONDecodeError as e:
                logger.error(f"Invalid JSON from WebSocket: {e}")

    except WebSocketDisconnect:
        ws_manager.disconnect(websocket)
        logger.info("WebSocket client disconnected from scan progress")
    except Exception as e:
        logger.error(f"WebSocket error: {e}")
        await websocket.close(code=1011, reason="Internal server error")


__all__ = ["api_v1_router"]
