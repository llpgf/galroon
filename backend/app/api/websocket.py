"""
WebSocket Manager for Galroon

Phase 3: Real-time scan progress updates via WebSocket.

This module provides WebSocket connections for:
- Scan progress updates
- Task notifications
- Real-time event broadcasting
"""

import logging
import json
from typing import Dict, Set
from fastapi import WebSocket, WebSocketDisconnect
from pydantic import BaseModel

logger = logging.getLogger(__name__)


class ScanProgressUpdate(BaseModel):
    """Scan progress update model."""
    current: int  # Current file being processed
    total: int  # Total files to scan
    percentage: float  # Progress percentage (0-100)
    message: str  # Status message
    is_complete: bool  # Whether scan is complete


class WebSocketManager:
    """
    Manages active WebSocket connections for real-time updates.
    """

    def __init__(self):
        self.active_connections: Set[WebSocket] = set()

    async def connect(self, websocket: WebSocket):
        """Accept a new WebSocket connection."""
        await websocket.accept()
        self.active_connections.add(websocket)
        logger.info(f"WebSocket connected. Total connections: {len(self.active_connections)}")

    def disconnect(self, websocket: WebSocket):
        """Remove a WebSocket connection."""
        self.active_connections.discard(websocket)
        logger.info(f"WebSocket disconnected. Total connections: {len(self.active_connections)}")

    async def broadcast_scan_progress(self, update: ScanProgressUpdate):
        """
        Broadcast scan progress to all connected clients.

        Args:
            update: Scan progress update data
        """
        message = {
            "type": "scan_progress",
            "data": update.dict()
        }

        disconnected = []
        for connection in self.active_connections:
            try:
                await connection.send_json(message)
            except Exception as e:
                logger.error(f"Error sending to WebSocket: {e}")
                disconnected.append(connection)

        # Clean up disconnected connections
        for connection in disconnected:
            self.disconnect(connection)

    async def broadcast_notification(self, notification_type: str, data: Dict):
        """
        Broadcast a notification to all connected clients.

        Args:
            notification_type: Type of notification
            data: Notification data
        """
        message = {
            "type": notification_type,
            "data": data
        }

        disconnected = []
        for connection in self.active_connections:
            try:
                await connection.send_json(message)
            except Exception as e:
                logger.error(f"Error sending notification: {e}")
                disconnected.append(connection)

        # Clean up disconnected connections
        for connection in disconnected:
            self.disconnect(connection)

    def get_connection_count(self) -> int:
        """Get the number of active connections."""
        return len(self.active_connections)


# Global WebSocket manager instance
ws_manager = WebSocketManager()


def get_ws_manager() -> WebSocketManager:
    """
    Get or create global WebSocket manager instance.

    Returns:
        WebSocketManager singleton
    """
    return ws_manager
