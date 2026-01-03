"""
Steam Connector for Galgame Library Manager.

**PHASE 12: External Integrations - Steam**

Provides integration with Steam Store API:
- Search for Steam App ID by game title
- Fetch high-resolution assets (header, hero, background)
- Download and cache images to .metadata/ directory
"""

import logging
import time
import requests
from pathlib import Path
from typing import Dict, Any, Optional, List
from urllib.parse import quote

logger = logging.getLogger(__name__)


class SteamConnector:
    """
    Connects to Steam Store API for game metadata and assets.

    Features:
    - Search Steam Store by title
    - Fetch high-resolution screenshots and headers
    - Download and cache assets locally
    - Rate limiting for API politeness
    """

    # Steam Store API endpoints
    STEAM_SEARCH_URL = "https://store.steampowered.com/api/storesearch/"
    STEAM_DETAILS_URL = "https://store.steampowered.com/api/appdetails"
    STEAM_AGE_CHECK_URL = "https://store.steampowered.com/agecheck/app/"

    def __init__(self, rate_limit_delay: float = 1.0):
        """
        Initialize Steam connector.

        Args:
            rate_limit_delay: Delay between API requests in seconds
        """
        self.rate_limit_delay = rate_limit_delay
        self.last_request_time = 0

    def _rate_limit(self):
        """Enforce rate limiting between requests."""
        current_time = time.time()
        time_since_last = current_time - self.last_request_time

        if time_since_last < self.rate_limit_delay:
            sleep_time = self.rate_limit_delay - time_since_last
            logger.debug(f"Rate limiting: sleeping for {sleep_time:.2f}s")
            time.sleep(sleep_time)

        self.last_request_time = time.time()

    def search_by_title(self, title: str) -> Optional[Dict[str, Any]]:
        """
        Search Steam Store for a game by title.

        Args:
            title: Game title to search for

        Returns:
            Dict with steam_id, name, and basic info, or None if not found
        """
        try:
            self._rate_limit()

            # Build search URL
            params = {
                "term": title,
                "l": "english",
                "cc": "US"
            }

            logger.info(f"Searching Steam for: {title}")

            response = requests.get(
                self.STEAM_SEARCH_URL,
                params=params,
                timeout=30
            )
            response.raise_for_status()

            data = response.json()

            # Check if we got results
            if not data.get("items"):
                logger.warning(f"No Steam results for: {title}")
                return None

            # Get first result
            items = data["items"]

            # Try to find exact match first
            exact_match = None
            for item in items:
                if item["name"].lower() == title.lower():
                    exact_match = item
                    break

            # Use exact match if found, otherwise use first result
            result = exact_match if exact_match else items[0]

            logger.info(f"Found Steam match: {result['name']} (ID: {result['id']})")

            return {
                "steam_id": str(result["id"]),
                "name": result["name"],
                "price": result.get("price"),
                "platforms": result.get("platforms"),
                "tiny_image": result.get("tiny_image")
            }

        except requests.exceptions.RequestException as e:
            logger.error(f"Steam search request failed: {e}")
            return None
        except Exception as e:
            logger.error(f"Error searching Steam: {e}")
            return None

    def get_app_details(self, steam_id: str) -> Optional[Dict[str, Any]]:
        """
        Get detailed app information from Steam.

        Args:
            steam_id: Steam App ID

        Returns:
            Dict with detailed app info or None
        """
        try:
            self._rate_limit()

            params = {
                "appids": steam_id,
                "l": "english"
            }

            logger.info(f"Fetching Steam app details: {steam_id}")

            response = requests.get(
                self.STEAM_DETAILS_URL,
                params=params,
                timeout=30
            )
            response.raise_for_status()

            data = response.json()

            # Check if app was found
            if steam_id not in data or not data[steam_id]["success"]:
                logger.warning(f"Steam app not found: {steam_id}")
                return None

            app_data = data[steam_id]["data"]

            return app_data

        except requests.exceptions.RequestException as e:
            logger.error(f"Steam details request failed: {e}")
            return None
        except Exception as e:
            logger.error(f"Error getting Steam details: {e}")
            return None

    def fetch_assets(
        self,
        steam_id: str,
        metadata_dir: Path,
        download: bool = True
    ) -> Dict[str, Optional[str]]:
        """
        Fetch and download high-resolution assets from Steam.

        Downloads:
        - Header image (capsule main)
        - Hero image (portrait)
        - Background image
        - Screenshots

        Args:
            steam_id: Steam App ID
            metadata_dir: .metadata/ directory to save assets
            download: If False, only return URLs without downloading

        Returns:
            Dict with local paths or URLs for all assets
        """
        try:
            # Get app details
            app_data = self.get_app_details(steam_id)

            if not app_data:
                return {}

            # Create metadata directory
            metadata_dir.mkdir(parents=True, exist_ok=True)

            assets = {}

            # Helper function to download image
            def download_image(url: str, filename: str) -> Optional[str]:
                """Download image from URL to local path."""
                if not download:
                    return url

                try:
                    self._rate_limit()

                    logger.debug(f"Downloading {filename} from Steam")
                    response = requests.get(url, timeout=30, stream=True)
                    response.raise_for_status()

                    dest_path = metadata_dir / filename

                    with open(dest_path, 'wb') as f:
                        for chunk in response.iter_content(chunk_size=8192):
                            if chunk:
                                f.write(chunk)

                    logger.info(f"Downloaded: {filename}")
                    return str(dest_path)

                except Exception as e:
                    logger.error(f"Failed to download {filename}: {e}")
                    return None

            # Extract and download header image (capsule)
            if "header_image" in app_data:
                assets["header"] = download_image(
                    app_data["header_image"],
                    "steam_header.jpg"
                )

            # Extract and download hero image (portrait)
            # This is usually the same as header but we try to get the background
            if "background" in app_data:
                assets["background"] = download_image(
                    app_data["background"],
                    "steam_background.jpg"
                )

            # Download screenshots (take up to 5)
            if "screenshots" in app_data and isinstance(app_data["screenshots"], list):
                screenshots = []
                for i, shot in enumerate(app_data["screenshots"][:5]):
                    if isinstance(shot, dict) and "path_full" in shot:
                        path = download_image(
                            shot["path_full"],
                            f"steam_screenshot_{i+1}.jpg"
                        )
                        if path:
                            screenshots.append(path)

                assets["screenshots"] = screenshots

            # Get additional screenshots if available
            if "movies" in app_data and isinstance(app_data["movies"], list):
                # We could extract thumbnails from videos
                pass

            return assets

        except Exception as e:
            logger.error(f"Error fetching Steam assets: {e}")
            return {}

    def search_and_fetch(
        self,
        title: str,
        metadata_dir: Path,
        download_assets: bool = True
    ) -> Dict[str, Any]:
        """
        Complete workflow: Search Steam and fetch assets.

        Args:
            title: Game title to search
            metadata_dir: .metadata/ directory
            download_assets: Whether to download assets

        Returns:
            Dict with steam_id, info, and assets
        """
        # Search for game
        search_result = self.search_by_title(title)

        if not search_result:
            return {
                "success": False,
                "steam_id": None,
                "assets": {}
            }

        steam_id = search_result["steam_id"]

        # Fetch assets
        assets = self.fetch_assets(
            steam_id,
            metadata_dir,
            download=download_assets
        )

        return {
            "success": True,
            "steam_id": steam_id,
            "name": search_result["name"],
            "price": search_result.get("price"),
            "platforms": search_result.get("platforms"),
            "assets": assets
        }


# Singleton instance
_steam_connector: Optional[SteamConnector] = None


def get_steam_connector(rate_limit_delay: float = 1.0) -> SteamConnector:
    """
    Get or create Steam connector singleton.

    Args:
        rate_limit_delay: Delay between requests in seconds

    Returns:
        SteamConnector instance
    """
    global _steam_connector
    if _steam_connector is None:
        _steam_connector = SteamConnector(rate_limit_delay=rate_limit_delay)
    return _steam_connector
