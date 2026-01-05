"""
Bangumi (番组计划) API Provider.

Bangumi is a Chinese ACG database covering anime, manga, and games.
API Documentation: https://github.com/bangumi/api
"""

import logging
import time
import asyncio
from typing import Optional, Dict, Any
import httpx

logger = logging.getLogger(__name__)


class BangumiProvider:
    """
    Bangumi API provider for game metadata.

    Uses Bangumi API v0: https://api.bgm.tv
    """

    API_BASE = "https://api.bgm.tv"
    RATE_LIMIT_DELAY = 1.0  # 1 request per second

    def __init__(self, client: Optional[httpx.AsyncClient] = None):
        """
        Initialize Bangumi provider.

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

    async def search_subjects(
        self,
        keyword: str,
        type: str = "4",
        response_group: str = "large"
    ) -> list[Dict[str, Any]]:
        """
        Search for subjects by keyword.

        Args:
            keyword: Search term
            type: Subject type (1=book, 2=anime, 3=music, 4=game, 6=real)
            response_group: Response size (small/medium/large)

        Returns:
            List of subject data
        """
        await self._rate_limit()

        try:
            response = await self.client.get(
                f"{self.API_BASE}/v0/search/subjects/{keyword}",
                params={
                    "type": type,
                    "responseGroup": response_group
                },
                headers={
                    "User-Agent": "GalgameLibraryManager/1.0"
                }
            )

            response.raise_for_status()
            data = response.json()

            return data.get("data", [])

        except httpx.HTTPStatusError as e:
            logger.error(f"Bangumi search error: {e.response.status_code}")
            return []
        except Exception as e:
            logger.error(f"Bangumi search failed: {e}")
            return []

    async def get_subject(self, subject_id: int) -> Optional[Dict[str, Any]]:
        """
        Get detailed subject information by ID.

        Args:
            subject_id: Bangumi subject ID

        Returns:
            Subject data or None if not found
        """
        await self._rate_limit()

        try:
            response = await self.client.get(
                f"{self.API_BASE}/v0/subjects/{subject_id}",
                params={"responseGroup": "large"},
                headers={
                    "User-Agent": "GalgameLibraryManager/1.0"
                }
            )

            response.raise_for_status()
            return response.json()

        except httpx.HTTPStatusError as e:
            if e.response.status_code == 404:
                return None
            logger.error(f"Bangumi get subject error: {e.response.status_code}")
            return None
        except Exception as e:
            logger.error(f"Bangumi get subject failed: {e}")
            return None

    async def get_subject_persons(self, subject_id: int) -> list[Dict[str, Any]]:
        """
        Get persons (staff) related to a subject.

        Args:
            subject_id: Bangumi subject ID

        Returns:
            List of person data
        """
        await self._rate_limit()

        try:
            response = await self.client.get(
                f"{self.API_BASE}/v0/subjects/{subject_id}/persons",
                headers={
                    "User-Agent": "GalgameLibraryManager/1.0"
                }
            )

            response.raise_for_status()
            data = response.json()

            return data

        except Exception as e:
            logger.error(f"Bangumi get persons failed: {e}")
            return []

    async def get_subject_characters(self, subject_id: int) -> list[Dict[str, Any]]:
        """
        Get characters related to a subject.

        Args:
            subject_id: Bangumi subject ID

        Returns:
            List of character data
        """
        await self._rate_limit()

        try:
            response = await self.client.get(
                f"{self.API_BASE}/v0/subjects/{subject_id}/characters",
                headers={
                    "User-Agent": "GalgameLibraryManager/1.0"
                }
            )

            response.raise_for_status()
            data = response.json()

            return data

        except Exception as e:
            logger.error(f"Bangumi get characters failed: {e}")
            return []

    def format_to_metadata(self, subject: Dict[str, Any]) -> Dict[str, Any]:
        """
        Format Bangumi subject data to standard metadata format.

        Args:
            subject: Raw Bangumi subject data

        Returns:
            Formatted metadata
        """
        # Extract basic info
        title = subject.get("name", "")
        summary = subject.get("summary", "")
        platform = subject.get("platform", [])

        # Get Chinese name if available
        name_cn = subject.get("name_cn", title)

        # Get infobox data
        infobox = subject.get("infobox", [])

        # Parse infobox for common fields
        developer = None
        release_date = None
        aliases = []

        for item in infobox:
            key = item.get("key", "")
            value = item.get("value", "")

            if key == "开发":
                developer = value
            elif key == "发行日期":
                release_date = value
            elif key == "别名":
                if isinstance(value, list):
                    aliases.extend([v.get("v", "") for v in value])
                else:
                    aliases.append(value)

        # Get images
        images = subject.get("images", {})
        cover_url = images.get("large", "") or images.get("common", "") or images.get("small", "")
        background_url = images.get("large", "")

        # Get rating
        rating = subject.get("rating", {})
        score = rating.get("score", 0)
        count = rating.get("total", 0)

        # Format to standard metadata structure
        return {
            "source": "bangumi",
            "source_id": str(subject.get("id", "")),
            "title": {
                "value": name_cn,
                "original": title,
                "en": "",
                "ja": "",
                "zh_hans": name_cn,
                "zh_hant": title
            },
            "description": {
                "value": summary
            },
            "developer": {
                "value": developer or "Unknown"
            },
            "release_date": {
                "value": release_date or ""
            },
            "platform": {
                "value": platform
            },
            "rating": {
                "value": {
                    "score": score,
                    "count": count
                }
            },
            "cover_url": {
                "value": cover_url
            },
            "background_url": {
                "value": background_url
            },
            "aliases": {
                "value": aliases
            },
            "tags": {
                "value": subject.get("tags", [])
            }
        }

    async def close(self):
        """Close the HTTP client."""
        await self.client.aclose()


def get_bangumi_provider() -> BangumiProvider:
    """
    Get or create Bangumi provider instance.

    Returns:
        BangumiProvider instance
    """
    return BangumiProvider()
