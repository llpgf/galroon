"""
Steam Store API Provider.

Steam is a digital distribution platform with many visual novels.
API Documentation: https://steamapi.xpaw.me/
"""

import logging
import time
import asyncio
from typing import Optional, Dict, Any
import httpx

logger = logging.getLogger(__name__)


class SteamProvider:
    """
    Steam Store API provider for game metadata.

    Uses Steam Store API for game information.
    """

    API_BASE = "https://store.steampowered.com/api"
    RATE_LIMIT_DELAY = 1.0  # 1 request per second (Steam recommends not exceeding 1 req/s)

    def __init__(self, client: Optional[httpx.AsyncClient] = None):
        """
        Initialize Steam provider.

        Args:
            client: Optional httpx client (for testing)
        """
        self.client = client or httpx.AsyncClient(timeout=30.0)
        self._last_request_time = 0.0

    async def _rate_limit(self):
        """Rate limiting: wait between requests."""
        current_time = time.time()
        time_since_last = current_time - self._last_request_time

        if time_since_last < self.RATE_LIMIT_DELAY:
            await asyncio.sleep(self.RATE_LIMIT_DELAY - time_since_last)

        self._last_request_time = time.time()

    async def search_app(self, keyword: str, lags: str = "english") -> list[Dict[str, Any]]:
        """
        Search for apps by keyword.

        Args:
            keyword: Search term
            lags: Language for results (default: english)

        Returns:
            List of app data
        """
        await self._rate_limit()

        try:
            response = await self.client.get(
                f"{self.API_BASE}/storesearch",
                params={
                    "term": keyword,
                    "l": "english",
                    "cc": "US"
                },
                headers={
                    "User-Agent": "GalgameLibraryManager/1.0"
                }
            )

            response.raise_for_status()
            data = response.json()

            return data.get("items", [])

        except httpx.HTTPStatusError as e:
            logger.error(f"Steam search error: {e.response.status_code}")
            return []
        except Exception as e:
            logger.error(f"Steam search failed: {e}")
            return []

    async def get_app_details(self, app_id: str, lags: str = "english") -> Optional[Dict[str, Any]]:
        """
        Get detailed app information by ID.

        Args:
            app_id: Steam app ID
            lags: Language for results (default: english)

        Returns:
            App data or None if not found
        """
        await self._rate_limit()

        try:
            response = await self.client.get(
                f"{self.API_BASE}/appdetails",
                params={
                    "appids": app_id,
                    "l": lags,
                    "cc": "US"
                },
                headers={
                    "User-Agent": "GalgameLibraryManager/1.0"
                }
            )

            response.raise_for_status()
            data = response.json()

            if app_id in data and data[app_id].get("success"):
                return data[app_id].get("data")

            return None

        except httpx.HTTPStatusError as e:
            if e.response.status_code == 404:
                return None
            logger.error(f"Steam get app error: {e.response.status_code}")
            return None
        except Exception as e:
            logger.error(f"Steam get app failed: {e}")
            return None

    def format_to_metadata(self, app: Dict[str, Any]) -> Dict[str, Any]:
        """
        Format Steam app data to standard metadata format.

        Args:
            app: Raw Steam app data

        Returns:
            Formatted metadata
        """
        # Extract basic info
        name = app.get("name", "")
        short_description = app.get("short_description", "")
        detailed_description = app.get("detailed_description", "")

        # Get developer/publisher
        developers = app.get("developers", [])
        publishers = app.get("publishers", [])
        developer = developers[0] if developers else (publishers[0] if publishers else "Unknown")

        # Get release date
        release_date = app.get("release_date", {})
        release_date_str = release_date.get("date", "") if release_date else ""

        # Get genres
        genres = app.get("genres", [])
        genre_list = [g.get("description", "") for g in genres] if genres else []

        # Get header image (cover)
        header_image = app.get("header_image", "")
        background = app.get("background", "")

        # Get screenshots
        screenshots = app.get("screenshots", [])
        screenshot_urls = [s.get("path_fullscreen", "") for s in screenshots] if screenshots else []

        # Get metacritic score
        metacritic = app.get("metacritic", {})
        score = metacritic.get("score", 0) if metacritic else 0

        # Get app ID
        steam_appid = app.get("steam_appid", "")

        # Get recommendations
        recommendations = app.get("recommendations", {})
        total_reviews = recommendations.get("total", 0) if recommendations else 0

        # Get price
        price_overview = app.get("price_overview")
        is_free = app.get("is_free", False)

        # Format to standard metadata structure
        return {
            "source": "steam",
            "source_id": str(steam_appid),
            "title": {
                "value": name,
                "original": name,
                "en": name,
                "ja": "",
                "zh_hans": name,
                "zh_hant": name
            },
            "description": {
                "value": detailed_description or short_description
            },
            "developer": {
                "value": developer
            },
            "release_date": {
                "value": release_date_str
            },
            "genres": {
                "value": genre_list
            },
            "rating": {
                "value": {
                    "score": score,
                    "count": total_reviews
                }
            },
            "cover_url": {
                "value": header_image
            },
            "background_url": {
                "value": background
            },
            "screenshots": {
                "value": screenshot_urls
            },
            "url": {
                "value": f"https://store.steampowered.com/app/{steam_appid}/"
            },
            "price": {
                "value": price_overview if not is_free else {"free": True}
            },
            "platform": {
                "value": ["steam", "windows", "pc"]
            }
        }

    async def close(self):
        """Close the HTTP client."""
        await self.client.aclose()


def get_steam_provider() -> SteamProvider:
    """
    Get or create Steam provider instance.

    Returns:
        SteamProvider instance
    """
    return SteamProvider()
