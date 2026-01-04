"""
VNDB API provider for visual novel metadata.

Implements fuzzy matching, rate limiting, and multilingual title support.
"""

import time
import logging
import re
from typing import Dict, List, Optional, Any
from pathlib import Path

try:
    from thefuzz import fuzz, process
    THEFUZZ_AVAILABLE = True
except ImportError:
    THEFUZZ_AVAILABLE = False
    logging.warning("thefuzz not available. Fuzzy matching will be disabled.")

import requests
from requests.adapters import HTTPAdapter
from urllib3.util.retry import Retry

from ..models import UnifiedMetadata, create_metadata_from_vndb
from ..normalizer import TextNormalizer, sanitize_description, normalize_rating

logger = logging.getLogger(__name__)


class VNDBProvider:
    """
    VNDB API provider with fuzzy matching and rate limiting.

    Features:
    - Fuzzy matching using thefuzz (Levenshtein distance)
    - Rate limiting (1 second between requests)
    - Multilingual title extraction
    - Automatic OpenCC conversion to Traditional Chinese
    - Error handling with retries
    """

    VNDB_API_URL = "https://api.vndb.org/kana"
    DEFAULT_RATE_LIMIT = 1.0  # seconds between requests

    def __init__(self, rate_limit: float = DEFAULT_RATE_LIMIT):
        """
        Initialize VNDB provider.

        Args:
            rate_limit: Seconds to wait between API requests
        """
        self.rate_limit = rate_limit
        self.last_request_time = 0.0
        self.session = self._create_session()

    def _create_session(self) -> requests.Session:
        """
        Create HTTP session with retry logic.

        Returns:
            Configured requests.Session
        """
        session = requests.Session()

        # Configure retry strategy
        retry_strategy = Retry(
            total=3,
            backoff_factor=1,
            status_forcelist=[429, 500, 502, 503, 504],
        )

        adapter = HTTPAdapter(max_retries=retry_strategy)
        session.mount("http://", adapter)
        session.mount("https://", adapter)

        # Set user agent
        session.headers.update({
            "User-Agent": "Galroon-Metadata-Manager/1.0",
            "Content-Type": "application/json"
        })

        return session

    def _rate_limit_wait(self):
        """Apply rate limiting by waiting if necessary."""
        current_time = time.time()
        time_since_last = current_time - self.last_request_time

        if time_since_last < self.rate_limit:
            wait_time = self.rate_limit - time_since_last
            logger.debug(f"Rate limiting: waiting {wait_time:.2f}s")
            time.sleep(wait_time)

        self.last_request_time = time.time()

    def _make_request(self, query: Dict[str, Any]) -> Optional[Dict]:
        """
        Make API request with rate limiting and error handling.

        Args:
            query: VNDB API query

        Returns:
            API response dict or None on error
        """
        self._rate_limit_wait()

        try:
            response = self.session.post(
                self.VNDB_API_URL,
                json=query,
                timeout=30
            )

            if response.status_code == 200:
                return response.json()
            elif response.status_code == 429:
                logger.warning("Rate limit exceeded, backing off...")
                time.sleep(5)
                return None
            else:
                logger.error(f"VNDB API error: {response.status_code} - {response.text}")
                return None

        except requests.exceptions.Timeout:
            logger.error("VNDB API request timed out")
            return None
        except requests.exceptions.RequestException as e:
            logger.error(f"VNDB API request failed: {e}")
            return None
        except Exception as e:
            logger.error(f"Unexpected error during VNDB request: {e}")
            return None

    def search_by_title(
        self,
        title: str,
        fuzzy_match: bool = True,
        min_score: int = 80
    ) -> Optional[Dict[str, Any]]:
        """
        Search VNDB by title with optional fuzzy matching.

        Args:
            title: Game title to search for
            fuzzy_match: If True, use fuzzy matching
            min_score: Minimum fuzzy match score (0-100)

        Returns:
            Best matching VNDB entry or None
        """
        if not title:
            return None

        # First try exact match
        query = {
            "query": f"search = \"{title}\"",
            "fields": self._get_search_fields(),
            "count": 5
        }

        logger.info(f"Searching VNDB for: {title}")
        results = self._make_request(query)

        if not results or "results" not in results:
            return None

        matches = results["results"]

        if not matches:
            logger.info(f"No exact matches found for: {title}")
            return None

        # If we got an exact match (score = 100)
        if len(matches) == 1 or not fuzzy_match:
            logger.info(f"Found exact match: {matches[0].get('title', 'Unknown')}")
            return matches[0]

        # Use fuzzy matching to find best match
        if THEFUZZ_AVAILABLE:
            best_match = self._fuzzy_find_best_match(title, matches, min_score)
            if best_match:
                logger.info(f"Fuzzy matched: {best_match.get('title', 'Unknown')} ({best_match['_fuzzy_score']}%)")
                return best_match

        # Fallback to first result
        return matches[0]

    def _fuzzy_find_best_match(
        self,
        target: str,
        matches: List[Dict],
        min_score: int
    ) -> Optional[Dict]:
        """
        Find best matching result using fuzzy string matching.

        Args:
            target: Target title
            matches: List of potential matches
            min_score: Minimum score threshold

        Returns:
            Best match with _fuzzy_score added, or None
        """
        # Extract titles from matches
        titles = [m.get("title", "") for m in matches]

        # Use fuzzy matching
        result = process.extractOne(
            target,
            titles,
            scorer=fuzz.WRatio
        )

        if not result:
            return None

        best_title, score, index = result

        if score < min_score:
            logger.info(f"Fuzzy match score {score} below threshold {min_score}")
            return None

        # Add fuzzy score to match
        best_match = matches[index]
        best_match["_fuzzy_score"] = score

        return best_match

    def get_metadata_by_id(self, vndb_id: str) -> Optional[Dict[str, Any]]:
        """
        Fetch complete metadata by VNDB ID.

        Args:
            vndb_id: VNDB identifier (e.g., "v12345")

        Returns:
            Complete metadata dict or None
        """
        # Ensure ID starts with 'v'
        if not vndb_id.startswith('v'):
            vndb_id = f'v{vndb_id}'

        query = {
            "query": f"id = {vndb_id}",
            "fields": self._get_detail_fields()
        }

        logger.info(f"Fetching VNDB metadata for: {vndb_id}")
        results = self._make_request(query)

        if not results or "results" not in results:
            return None

        if len(results["results"]) == 0:
            logger.warning(f"No results found for VNDB ID: {vndb_id}")
            return None

        return results["results"][0]

    def fetch_and_parse(
        self,
        game_name: str,
        prefer_traditional: bool = True
    ) -> Optional[UnifiedMetadata]:
        """
        Fetch and parse metadata for a game.

        Args:
            game_name: Game title to search for
            prefer_traditional: Convert to Traditional Chinese

        Returns:
            UnifiedMetadata object or None
        """
        # Search for game
        match = self.search_by_title(game_name)

        if not match:
            return None

        # Get full metadata
        vndb_id = match.get("id")
        if not vndb_id:
            return None

        full_data = self.get_metadata_by_id(vndb_id)
        if not full_data:
            return None

        # Parse into UnifiedMetadata
        metadata = self._parse_vndb_data(full_data, prefer_traditional)

        return metadata

    def _parse_vndb_data(
        self,
        data: Dict[str, Any],
        prefer_traditional: bool
    ) -> UnifiedMetadata:
        """
        Parse VNDB API data into UnifiedMetadata.

        Args:
            data: Raw VNDB API response
            prefer_traditional: Convert to Traditional Chinese

        Returns:
            UnifiedMetadata object
        """
        from ..models import MetadataField, MultilingualTitle, Rating

        # Extract titles
        titles = self._extract_titles(data, prefer_traditional)

        # Extract description
        description_raw = data.get("description", "")
        description = sanitize_description(description_raw)

        # Extract rating (VNDB uses 0-10 scale already)
        rating_raw = data.get("rating", 0.0)
        rating = normalize_rating(rating_raw, scale_max=10.0)

        # Extract tags
        tags = self._extract_tags(data)

        # Extract cover image
        cover_url = ""
        if data.get("images"):
            images = data["images"]
            if isinstance(images, dict) and images.get("cover"):
                cover_url = images["cover"].get("url", "")

        # Extract screenshots
        screenshot_urls = []
        if data.get("images"):
            images = data["images"]
            if isinstance(images, dict) and images.get("screenshots"):
                screenshots = images["screenshots"]
                screenshot_urls = [s.get("url", "") for s in screenshots if isinstance(s, dict)]

        # Extract release date
        release_date = ""
        if data.get("released"):
            release_date = data["released"]  # VNDB uses YYYY-MM-DD format

        # Extract developer
        developer = ""
        if data.get("developers"):
            developers = data["developers"]
            if isinstance(developers, list) and len(developers) > 0:
                developer = developers[0].get("name", "")

        # Extract characters (cast list)
        characters_list = self._extract_characters(data, max_characters=5)

        # Extract staff
        staff_list = self._extract_staff(data, max_staff=10)

        # Pick best background image
        background_url = self._pick_best_background(data)

        # Build multilingual title
        multi_title = MultilingualTitle(**titles)

        # Create rating object
        rating_obj = Rating(score=rating, count=int(data.get("votecount", 0)), source="vndb")

        # Create UnifiedMetadata
        return UnifiedMetadata(
            vndb_id=data.get("id", ""),
            title=MetadataField(value=multi_title, source="vndb"),
            description=MetadataField(value=description, source="vndb"),
            rating=MetadataField(value=rating_obj, source="vndb"),
            cover_url=MetadataField(value=cover_url, source="vndb"),
            screenshot_urls=MetadataField(value=screenshot_urls, source="vndb"),
            background_url=MetadataField(value=background_url, source="vndb"),
            characters=MetadataField(value=characters_list, source="vndb"),
            staff=MetadataField(value=staff_list, source="vndb"),
            tags=MetadataField(value=tags, source="vndb"),
            release_date=MetadataField(value=release_date, source="vndb"),
            developer=MetadataField(value=developer, source="vndb"),
            providers=["vndb"]
        )

    def _extract_titles(self, data: Dict, prefer_traditional: bool) -> Dict[str, str]:
        """
        Extract titles in multiple languages.

        Args:
            data: VNDB API response
            prefer_traditional: Convert Chinese to Traditional

        Returns:
            Dictionary of titles
        """
        titles = {
            "ja": "",
            "en": "",
            "zh_hans": "",
            "zh_hant": "",
            "original": ""
        }

        # Original title (usually Japanese)
        titles["original"] = data.get("original", "")
        titles["ja"] = titles["original"]

        # Main title (often English or Romanized)
        titles["en"] = data.get("title", "")

        # Alternative titles
        alt_titles = data.get("alttitles", [])
        for alt in alt_titles:
            if not isinstance(alt, dict):
                continue

            title_text = alt.get("title", "")
            lang = alt.get("lang", "")

            # Map VNDB language codes
            if lang == "zh-Hans":
                titles["zh_hans"] = title_text
                # Convert to Traditional
                if prefer_traditional:
                    titles["zh_hant"] = TextNormalizer.to_traditional_chinese(title_text)
            elif lang == "zh-Hant":
                titles["zh_hant"] = title_text

        # Fallback: if no Chinese titles, try to convert English
        if not titles["zh_hant"] and not titles["zh_hans"] and titles["en"]:
            # This won't work well, but it's a fallback
            titles["zh_hans"] = titles["en"]
            if prefer_traditional:
                titles["zh_hant"] = titles["en"]

        return titles

    def _extract_tags(self, data: Dict) -> List[str]:
        """
        Extract and normalize tags.

        Args:
            data: VNDB API response

        Returns:
            List of tag names
        """
        tags = []

        # VNDB uses "tags" array with objects containing "name"
        raw_tags = data.get("tags", [])
        for tag in raw_tags:
            if isinstance(tag, dict):
                tag_name = tag.get("name", "")
                if tag_name:
                    tags.append(tag_name)

        # Normalize tags
        return TextNormalizer.normalize_tags(tags)

    def _extract_characters(self, data: Dict, max_characters: int = 5) -> List[Dict]:
        """
        Extract character information from VNDB data.

        Args:
            data: VNDB API response
            max_characters: Maximum number of characters to extract

        Returns:
            List of character dictionaries
        """
        from ..models import Character

        characters = []
        raw_characters = data.get("characters", [])

        for char_data in raw_characters[:max_characters]:
            if not isinstance(char_data, dict):
                continue

            # Extract character name
            name = char_data.get("name", "")

            # Only include main characters (with spoil=0)
            if char_data.get("spoil", 1) > 0:
                continue

            # Extract voice actor (CV)
            cv = None
            voicers = char_data.get("voicers", [])
            if voicers and len(voicers) > 0:
                # Get first VA
                cv = voicers[0].get("name", None) if isinstance(voicers[0], dict) else None

            # Extract role
            role = char_data.get("role", "")
            role_map = {
                "main": "protagonist",
                "primary": "main",
                "secondary": "supporting",
            }
            role = role_map.get(role, role)

            # Extract image
            image_data = char_data.get("image")
            image_url = None
            if isinstance(image_data, dict):
                image_url = image_data.get("url", None)

            character = {
                "name": name,
                "name_ja": None,  # VNDB doesn't always provide this separately
                "role": role,
                "cv": cv,
                "image_url": image_url,
                "description": None,
                "spoiler": False
            }

            characters.append(character)

        return characters

    def _extract_staff(self, data: Dict, max_staff: int = 10) -> List[Dict]:
        """
        Extract staff information from VNDB data.

        Args:
            data: VNDB API response
            max_staff: Maximum number of staff members to extract

        Returns:
            List of staff dictionaries
        """
        from ..models import Staff

        staff = []
        raw_staff = data.get("staff", [])

        for staff_data in raw_staff[:max_staff]:
            if not isinstance(staff_data, dict):
                continue

            # Extract staff name
            name = ""
            if isinstance(staff_data.get("name"), dict):
                name = staff_data["name"].get("name", "")
            else:
                name = staff_data.get("name", "")

            # Extract role (e.g., "Scenario", "Art", "Music")
            role = ""
            aliases = staff_data.get("aliases", [])
            if isinstance(aliases, list) and len(aliases) > 0:
                role = aliases[0].get("name", "")
            elif isinstance(staff_data.get("role"), dict):
                role = staff_data["role"].get("name", "")
            else:
                role = staff_data.get("role", "")

            # Note: EID (external ID) or other info
            note = staff_data.get("note", None)

            staff_member = {
                "name": name,
                "role": role,
                "note": note
            }

            staff.append(staff_member)

        return staff

    def _pick_best_background(self, data: Dict) -> Optional[str]:
        """
        Pick the best screenshot for background from VNDB data.

        Strategy: Use the first screenshot (usually highest rated/promoted).

        Args:
            data: VNDB API response

        Returns:
            URL to best background image or None
        """
        if not data.get("images"):
            return None

        images = data["images"]
        if not isinstance(images, dict):
            return None

        # Try screenshots first
        screenshots = images.get("screenshots", [])
        if isinstance(screenshots, list) and len(screenshots) > 0:
            # Pick first screenshot (usually highest quality)
            first_shot = screenshots[0]
            if isinstance(first_shot, dict):
                return first_shot.get("url", "")

        # Fallback to cover image
        cover = images.get("cover")
        if isinstance(cover, dict):
            return cover.get("url", "")

        return None

    def _get_search_fields(self) -> str:
        """Get fields for search queries."""
        return "id, title, original, released"

    def _get_detail_fields(self) -> str:
        """Get fields for detail queries."""
        return (
            "id, title, original, alttitles, description, released, "
            "languages, platforms, image, images, rating, votecount, "
            "tags, developers, staff, characters"
        )


# Singleton instance
_vndb_provider: Optional[VNDBProvider] = None


def get_vndb_provider(rate_limit: float = 1.0) -> VNDBProvider:
    """
    Get or create VNDB provider singleton.

    Args:
        rate_limit: Rate limit in seconds

    Returns:
        VNDBProvider instance
    """
    global _vndb_provider
    if _vndb_provider is None:
        _vndb_provider = VNDBProvider(rate_limit=rate_limit)
    return _vndb_provider
