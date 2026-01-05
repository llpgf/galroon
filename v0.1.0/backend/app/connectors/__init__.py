"""
Connectors package for Galgame Library Manager.

**PHASE 12: The Connector - External Integrations**

Provides:
- Steam Connector (high-res assets)
- Bangumi Connector (Chinese metadata)
"""

from .steam import SteamConnector, get_steam_connector
from .bangumi import BangumiConnector, get_bangumi_connector

__all__ = [
    "SteamConnector",
    "get_steam_connector",
    "BangumiConnector",
    "get_bangumi_connector",
]
