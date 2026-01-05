"""
Metadata providers module.

Supports multiple metadata sources with extensible architecture.
"""

from .vndb import VNDBProvider, get_vndb_provider
from .bangumi import BangumiProvider, get_bangumi_provider
from .erogamescape import ErogameScapeProvider, get_erogamescape_provider
from .steam import SteamProvider, get_steam_provider

__all__ = [
    "VNDBProvider",
    "get_vndb_provider",
    "BangumiProvider",
    "get_bangumi_provider",
    "ErogameScapeProvider",
    "get_erogamescape_provider",
    "SteamProvider",
    "get_steam_provider",
]
