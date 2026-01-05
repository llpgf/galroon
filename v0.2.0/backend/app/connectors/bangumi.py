"""
Bangumi Connector for Galgame Library Manager.

**PHASE 12: External Integrations - Bangumi**

Provides integration with Bangumi (bgm.tv) API:
- Search for visual novels by Chinese/Japanese title
- Fetch Chinese description and rating
- Primary source for Chinese metadata when VNDB lacks it
"""

import logging
import time
import requests
from typing import Dict, Any, Optional, List
from urllib.parse import quote

logger = logging.getLogger(__name__)


class BangumiConnector:
    """
    Connects to Bangumi API for Chinese metadata.

    Features:
    - Search by Chinese/Japanese title
    - Fetch Chinese description (summary)
    - Get rating information
    - Rate limiting for API politeness
    """

    # Bangumi API endpoints (v0)
    BANGUMI_API_BASE = "https://api.bgm.tv"
    SEARCH_URL = "{}/search/subject/{}"
    SUBJECT_URL = "{}/subject/{}"

    # Bangumi subject type for games (4 = Game)
    SUBJECT_TYPE_GAME = 4

    def __init__(self, rate_limit_delay: float = 1.0, user_agent: str = None):
        """
        Initialize Bangumi connector.

        Args:
            rate_limit_delay: Delay between API requests in seconds
            user_agent: Custom user agent string
        """
        self.rate_limit_delay = rate_limit_delay
        self.last_request_time = 0

        # Bangumi requires a User-Agent header
        self.headers = {
            "User-Agent": user_agent or "GalgameLibraryManager/1.0 (github.com/user/repo)",
            "Accept": "application/json"
        }

    def _rate_limit(self):
        """Enforce rate limiting between requests."""
        current_time = time.time()
        time_since_last = current_time - self.last_request_time

        if time_since_last < self.rate_limit_delay:
            sleep_time = self.rate_limit_delay - time_since_last
            logger.debug(f"Rate limiting: sleeping for {sleep_time:.2f}s")
            time.sleep(sleep_time)

        self.last_request_time = time.time()

    def search_by_title(
        self,
        title: str,
        max_results: int = 5
    ) -> List[Dict[str, Any]]:
        """
        Search Bangumi for a game by title.

        Args:
            title: Game title to search (Chinese or Japanese)
            max_results: Maximum number of results to return

        Returns:
            List of dicts with bangumi_id, name, and summary
        """
        try:
            self._rate_limit()

            # URL encode the title
            encoded_title = quote(title)

            logger.info(f"Searching Bangumi for: {title}")

            response = requests.get(
                self.SEARCH_URL.format(self.BANGUMI_API_BASE, encoded_title),
                headers=self.headers,
                params={
                    "type": self.SUBJECT_TYPE_GAME,
                    "responseGroup": "large"
                },
                timeout=30
            )
            response.raise_for_status()

            data = response.json()

            # Check if we got results
            if not data.get("list"):
                logger.warning(f"No Bangumi results for: {title}")
                return []

            results = []
            for item in data["list"][:max_results]:
                # Parse item
                result = {
                    "bangumi_id": str(item["id"]),
                    "name": item.get("name", ""),
                    "name_cn": item.get("name_cn", ""),
                    "score": item.get("rating", {}).get("score", 0.0),
                    "rating_count": item.get("rating", {}).get("total", 0),
                    "summary": item.get("summary", ""),  # This is Chinese description
                    "images": item.get("images", {})
                }
                results.append(result)

            logger.info(f"Found {len(results)} Bangumi results for: {title}")
            return results

        except requests.exceptions.RequestException as e:
            logger.error(f"Bangumi search request failed: {e}")
            return []
        except Exception as e:
            logger.error(f"Error searching Bangumi: {e}")
            return []

    def get_subject_details(self, bangumi_id: str) -> Optional[Dict[str, Any]]:
        """
        Get detailed subject information from Bangumi.

        Args:
            bangumi_id: Bangumi subject ID

        Returns:
            Dict with detailed subject info or None
        """
        try:
            self._rate_limit()

            logger.info(f"Fetching Bangumi subject: {bangumi_id}")

            response = requests.get(
                self.SUBJECT_URL.format(self.BANGUMI_API_BASE, bangumi_id),
                headers=self.headers,
                timeout=30
            )
            response.raise_for_status()

            data = response.json()

            # Extract relevant fields
            result = {
                "bangumi_id": bangumi_id,
                "name": data.get("name", ""),
                "name_cn": data.get("name_cn", ""),
                "summary": data.get("summary", ""),  # Chinese description
                "summary_cn": data.get("summary", ""),  # Alias
                "score": data.get("rating", {}).get("score", 0.0),
                "rating_count": data.get("rating", {}).get("total", 0),
                "rank": data.get("rating", {}).get("rank", 0),
                "images": data.get("images", {}),
                "tags": [
                    {"name": tag.get("name"), "count": tag.get("count")}
                    for tag in data.get("tags", [])
                ],
                "info": data.get("info", {})
            }

            return result

        except requests.exceptions.RequestException as e:
            logger.error(f"Bangumi details request failed: {e}")
            return None
        except Exception as e:
            logger.error(f"Error getting Bangumi details: {e}")
            return None

    def find_best_match(
        self,
        title: str,
        title_cn: Optional[str] = None,
        title_ja: Optional[str] = None,
        max_results: int = 10
    ) -> Optional[Dict[str, Any]]:
        """
        Find the best matching game on Bangumi using multiple title variants.

        Tries to match in order:
        1. Chinese title (if available)
        2. Japanese title
        3. Original title

        Args:
            title: Original title
            title_cn: Chinese title (if available)
            title_ja: Japanese title (if available)
            max_results: Maximum search results to consider

        Returns:
            Best matching subject or None
        """
        # Try Chinese title first
        if title_cn:
            results = self.search_by_title(title_cn, max_results=max_results)
            if results:
                return results[0]

        # Try Japanese title
        if title_ja:
            results = self.search_by_title(title_ja, max_results=max_results)
            if results:
                return results[0]

        # Try original title
        if title:
            results = self.search_by_title(title, max_results=max_results)
            if results:
                return results[0]

        logger.warning(f"No Bangumi match found for: {title}")
        return None

    def get_chinese_metadata(
        self,
        title: str,
        title_cn: Optional[str] = None,
        title_ja: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Complete workflow: Search Bangumi and extract Chinese metadata.

        Args:
            title: Original game title
            title_cn: Chinese title (if available)
            title_ja: Japanese title (if available)

        Returns:
            Dict with bangumi_id, summary_cn, rating_score
        """
        # Find best match
        best_match = self.find_best_match(
            title=title,
            title_cn=title_cn,
            title_ja=title_ja
        )

        if not best_match:
            return {
                "success": False,
                "bangumi_id": None,
                "summary_cn": None,
                "rating_score": 0.0
            }

        # Get detailed info
        subject_id = best_match["bangumi_id"]
        details = self.get_subject_details(subject_id)

        if not details:
            return {
                "success": True,
                "bangumi_id": subject_id,
                "summary_cn": best_match.get("summary"),
                "rating_score": best_match.get("score", 0.0)
            }

        return {
            "success": True,
            "bangumi_id": subject_id,
            "name_cn": details.get("name_cn"),
            "summary_cn": details.get("summary"),
            "rating_score": details.get("score", 0.0),
            "rating_count": details.get("rating_count", 0),
            "rank": details.get("rank", 0),
            "tags": details.get("tags", []),
            "cover_image": details.get("images", {}).get("large")
        }


# Singleton instance
_bangumi_connector: Optional[BangumiConnector] = None


def get_bangumi_connector(
    rate_limit_delay: float = 1.0,
    user_agent: str = None
) -> BangumiConnector:
    """
    Get or create Bangumi connector singleton.

    Args:
        rate_limit_delay: Delay between requests in seconds
        user_agent: Custom user agent string

    Returns:
        BangumiConnector instance
    """
    global _bangumi_connector
    if _bangumi_connector is None:
        _bangumi_connector = BangumiConnector(
            rate_limit_delay=rate_limit_delay,
            user_agent=user_agent
        )
    return _bangumi_connector
