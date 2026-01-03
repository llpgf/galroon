"""
ErogameScape API Provider.

ErogameScape is a Japanese eroge rating and review site.
API Documentation: https://github.com/faryne/api/blob/master/erogamescape_api.md
"""

import logging
import time
import asyncio
from typing import Optional, Dict, Any
import httpx

logger = logging.getLogger(__name__)


class ErogameScapeProvider:
    """
    ErogameScape API provider for game metadata.

    Uses ErogameScape API for game information and ratings.
    """

    API_BASE = "https://api.erogamescape.com"
    RATE_LIMIT_DELAY = 1.0  # 1 request per second

    def __init__(self, client: Optional[httpx.AsyncClient] = None):
        """
        Initialize ErogameScape provider.

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

    async def search_game(self, keyword: str) -> list[Dict[str, Any]]:
        """
        Search for games by keyword.

        Args:
            keyword: Search term

        Returns:
            List of game data
        """
        await self._rate_limit()

        try:
            response = await self.client.get(
                f"{self.API_BASE}/api/game",
                params={"keyword": keyword},
                headers={
                    "User-Agent": "GalgameLibraryManager/1.0"
                }
            )

            response.raise_for_status()
            data = response.json()

            return data.get("games", []) if isinstance(data, dict) else data

        except httpx.HTTPStatusError as e:
            logger.error(f"ErogameScape search error: {e.response.status_code}")
            return []
        except Exception as e:
            logger.error(f"ErogameScape search failed: {e}")
            return []

    async def get_game(self, game_id: str) -> Optional[Dict[str, Any]]:
        """
        Get detailed game information by ID.

        Args:
            game_id: ErogameScape game ID

        Returns:
            Game data or None if not found
        """
        await self._rate_limit()

        try:
            response = await self.client.get(
                f"{self.API_BASE}/api/game",
                params={"id": game_id},
                headers={
                    "User-Agent": "GalgameLibraryManager/1.0"
                }
            )

            response.raise_for_status()
            data = response.json()

            if isinstance(data, dict) and "games" in data:
                games = data["games"]
                return games[0] if len(games) > 0 else None

            return None

        except httpx.HTTPStatusError as e:
            if e.response.status_code == 404:
                return None
            logger.error(f"ErogameScape get game error: {e.response.status_code}")
            return None
        except Exception as e:
            logger.error(f"ErogameScape get game failed: {e}")
            return None

    def format_to_metadata(self, game: Dict[str, Any]) -> Dict[str, Any]:
        """
        Format ErogameScape game data to standard metadata format.

        Args:
            game: Raw ErogameScape game data

        Returns:
            Formatted metadata
        """
        # Extract basic info
        title = game.get("title", "")
        title_jp = game.get("title_jp", title)

        # Get brand/developer
        brand = game.get("brand", "")
        developer = brand or "Unknown"

        # Get release date
        release_date = game.get("sellday", "")
        release_date_str = str(release_date) if release_date else ""

        # Get description
        description = game.get("description", "")

        # Get genre
        genre = game.get("genre", [])
        if isinstance(genre, str):
            genre = [genre]

        # Get rating
        median = game.get("median", None)
        rating_count = game.get("count_num", 0)

        # Get URL
        eroges_url = game.get("url", "")
        official_url = game.get("official_url", "")

        # Get images (if any)
        image_url = game.get("image_url", "")

        # Format to standard metadata structure
        return {
            "source": "erogamescape",
            "source_id": str(game.get("id", "")),
            "title": {
                "value": title_jp,
                "original": title_jp,
                "en": title,
                "ja": title_jp,
                "zh_hans": title,
                "zh_hant": title
            },
            "description": {
                "value": description
            },
            "developer": {
                "value": developer
            },
            "release_date": {
                "value": release_date_str
            },
            "genres": {
                "value": genre
            },
            "rating": {
                "value": {
                    "score": median or 0,
                    "count": rating_count
                }
            },
            "cover_url": {
                "value": image_url
            },
            "background_url": {
                "value": image_url
            },
            "url": {
                "value": eroges_url
            },
            "official_url": {
                "value": official_url
            },
            "tags": {
                "value": genre
            }
        }

    async def close(self):
        """Close the HTTP client."""
        await self.client.aclose()


def get_erogamescape_provider() -> ErogameScapeProvider:
    """
    Get or create ErogameScape provider instance.

    Returns:
        ErogameScapeProvider instance
    """
    return ErogameScapeProvider()
