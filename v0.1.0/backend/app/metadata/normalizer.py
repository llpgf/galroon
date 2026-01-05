"""
Data normalization utilities for metadata processing.

Provides HTML cleaning, rating normalization, and OpenCC conversion.
"""

import re
import html
import logging
from typing import Optional

try:
    import opencc
    OPENCC_AVAILABLE = True
except ImportError:
    OPENCC_AVAILABLE = False
    logging.warning("OpenCC not available. Chinese conversion will be disabled.")

logger = logging.getLogger(__name__)


class TextNormalizer:
    """
    Text normalization utilities.

    Handles HTML cleaning, whitespace normalization, and Chinese conversion.
    """

    # OpenCC converter for Simplified -> Traditional Chinese
    _s2t_converter: Optional['opencc.OpenCC'] = None
    _t2s_converter: Optional['opencc.OpenCC'] = None

    @classmethod
    def _get_s2t_converter(cls) -> Optional['opencc.OpenCC']:
        """Get or create Simplified to Traditional converter."""
        if not OPENCC_AVAILABLE:
            return None
        if cls._s2t_converter is None:
            try:
                cls._s2t_converter = opencc.OpenCC('s2t.json')  # Simplified to Traditional
            except Exception as e:
                logger.error(f"Failed to initialize OpenCC s2t converter: {e}")
                return None
        return cls._s2t_converter

    @classmethod
    def _get_t2s_converter(cls) -> Optional['opencc.OpenCC']:
        """Get or create Traditional to Simplified converter."""
        if not OPENCC_AVAILABLE:
            return None
        if cls._t2s_converter is None:
            try:
                cls._t2s_converter = opencc.OpenCC('t2s.json')  # Traditional to Simplified
            except Exception as e:
                logger.error(f"Failed to initialize OpenCC t2s converter: {e}")
                return None
        return cls._t2s_converter

    @staticmethod
    def clean_html(html_content: str) -> str:
        """
        Remove HTML tags and decode HTML entities.

        Preserves line breaks and paragraph structure.

        Args:
            html_content: HTML content to clean

        Returns:
            Plain text with HTML removed
        """
        if not html_content:
            return ""

        # Decode HTML entities
        text = html.unescape(html_content)

        # Remove HTML tags
        # Replace <br> and </p> with newlines
        text = re.sub(r'<br\s*/?>', '\n', text)
        text = re.sub(r'</\s*p\s*>', '\n\n', text)

        # Remove all other tags
        text = re.sub(r'<[^>]+>', '', text)

        # Normalize whitespace
        text = re.sub(r'\n{3,}', '\n\n', text)  # Max 2 consecutive newlines
        text = re.sub(r'[ \t]+', ' ', text)  # Normalize spaces

        # Strip leading/trailing whitespace
        text = text.strip()

        return text

    @staticmethod
    def normalize_rating(rating: float, scale_max: float = 100.0) -> float:
        """
        Normalize rating from arbitrary scale to 0-10 scale.

        Args:
            rating: Rating value to normalize
            scale_max: Maximum value of input scale (default: 100)

        Returns:
            Rating normalized to 0-10 scale
        """
        if not isinstance(rating, (int, float)):
            return 0.0

        # Clamp to valid range
        rating = max(0.0, min(rating, scale_max))

        # Normalize to 0-10
        normalized = (rating / scale_max) * 10.0

        # Round to 1 decimal place
        return round(normalized, 1)

    @staticmethod
    def truncate_text(text: str, max_length: int = 500, suffix: str = "...") -> str:
        """
        Truncate text to maximum length while preserving word boundaries.

        Args:
            text: Text to truncate
            max_length: Maximum length
            suffix: Suffix to add if truncated

        Returns:
            Truncated text
        """
        if not text or len(text) <= max_length:
            return text

        # Truncate at word boundary
        truncated = text[:max_length]
        last_space = truncated.rfind(' ')

        if last_space > max_length * 0.8:  # If last space is in reasonable position
            truncated = truncated[:last_space]

        return truncated + suffix

    @classmethod
    def to_traditional_chinese(cls, text: str) -> str:
        """
        Convert Simplified Chinese to Traditional Chinese.

        Args:
            text: Text to convert

        Returns:
            Traditional Chinese text (or original if conversion fails)
        """
        if not text:
            return text

        converter = cls._get_s2t_converter()
        if converter is None:
            logger.warning("OpenCC not available, returning original text")
            return text

        try:
            return converter.convert(text)
        except Exception as e:
            logger.error(f"OpenCC conversion failed: {e}")
            return text

    @classmethod
    def to_simplified_chinese(cls, text: str) -> str:
        """
        Convert Traditional Chinese to Simplified Chinese.

        Args:
            text: Text to convert

        Returns:
            Simplified Chinese text (or original if conversion fails)
        """
        if not text:
            return text

        converter = cls._get_t2s_converter()
        if converter is None:
            logger.warning("OpenCC not available, returning original text")
            return text

        try:
            return converter.convert(text)
        except Exception as e:
            logger.error(f"OpenCC conversion failed: {e}")
            return text

    @staticmethod
    def clean_filename(filename: str) -> str:
        """
        Clean filename by removing invalid characters.

        Args:
            filename: Filename to clean

        Returns:
            Cleaned filename
        """
        # Remove invalid characters
        filename = re.sub(r'[<>:"/\\|?*]', '', filename)

        # Replace multiple spaces with single space
        filename = re.sub(r'\s+', ' ', filename)

        # Trim whitespace and dots
        filename = filename.strip('. ')

        # Ensure filename is not empty
        if not filename:
            filename = "unnamed"

        return filename

    @staticmethod
    def extract_year(release_date: str) -> Optional[str]:
        """
        Extract year from release date string.

        Args:
            release_date: Date string (YYYY-MM-DD, YYYY, or various formats)

        Returns:
            Year as string or None
        """
        if not release_date:
            return None

        # Try to extract year (4 digits)
        match = re.search(r'\b(19\d{2}|20\d{2})\b', release_date)
        if match:
            return match.group(1)

        return None

    @staticmethod
    def normalize_tags(tags: list) -> list:
        """
        Normalize tag names.

        - Trim whitespace
        - Remove duplicates
        - Sort alphabetically

        Args:
            tags: List of tag names

        Returns:
            Normalized list of tags
        """
        if not tags:
            return []

        # Trim and filter empty
        normalized = [tag.strip() for tag in tags if tag and tag.strip()]

        # Remove duplicates (case-insensitive)
        seen = set()
        unique_tags = []
        for tag in normalized:
            tag_lower = tag.lower()
            if tag_lower not in seen:
                seen.add(tag_lower)
                unique_tags.append(tag)

        # Sort alphabetically
        return sorted(unique_tags)

    @staticmethod
    def sanitize_description(description: str, max_length: int = 5000) -> str:
        """
        Sanitize game description.

        - Clean HTML
        - Remove excessive whitespace
        - Truncate if too long

        Args:
            description: Description text
            max_length: Maximum length

        Returns:
            Sanitized description
        """
        if not description:
            return ""

        # Clean HTML
        clean = TextNormalizer.clean_html(description)

        # Truncate if too long
        if len(clean) > max_length:
            clean = TextNormalizer.truncate_text(clean, max_length, "...\n[Description truncated]")

        return clean


# Convenience functions for common operations

def clean_html(html_content: str) -> str:
    """Clean HTML from content (convenience function)."""
    return TextNormalizer.clean_html(html_content)


def normalize_rating(rating: float, scale_max: float = 100.0) -> float:
    """Normalize rating to 0-10 scale (convenience function)."""
    return TextNormalizer.normalize_rating(rating, scale_max)


def to_traditional_chinese(text: str) -> str:
    """Convert to Traditional Chinese (convenience function)."""
    return TextNormalizer.to_traditional_chinese(text)


def to_simplified_chinese(text: str) -> str:
    """Convert to Simplified Chinese (convenience function)."""
    return TextNormalizer.to_simplified_chinese(text)


def sanitize_description(description: str, max_length: int = 5000) -> str:
    """Sanitize description (convenience function)."""
    return TextNormalizer.sanitize_description(description, max_length)
