"""
API v1 Endpoints

This module aggregates all API v1 routes with a common /api/v1 prefix.

Phase 3: Added WebSocket support for real-time updates
"""

from fastapi import APIRouter, WebSocket, WebSocketDisconnect
from fastapi.responses import HTMLResponse

# Import all existing routers
from . import (
    analytics_router,
    backup_router,
    connectors_router,
    curator_router,
    games_router,
    history_router,
    image_cache_api_router,
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

# Import WebSocket manager
from ..websocket import get_ws_manager, ScanProgressUpdate

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
    import logging
    ws_manager = get_ws_manager()
    logger = logging.getLogger(__name__)

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
