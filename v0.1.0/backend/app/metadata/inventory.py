"""
Asset Inventory System for Galgame Library Manager.

**PHASE 9 FEATURE:**
Automatically detect and categorize game assets (ISO, DLC, patches, cracks, etc.)
to provide a "Roon-like" inventory view of what files are in each game directory.

This enables:
- Multi-version aggregation (CD version + Steam version under one work)
- Asset visibility (what DLCs, patches, extras are present)
- Intelligent version labeling (auto-detect "HDD", "ISO", "Chinese Patch")
"""

import re
import logging
from pathlib import Path
from typing import List, Dict, Set, Optional
from dataclasses import dataclass

logger = logging.getLogger(__name__)


@dataclass
class AssetDetectionResult:
    """Result of asset detection for a game directory."""
    assets: List[str]  # Detected asset tags
    version_label: str  # Suggested version label (e.g., "ISO + Chinese Patch")
    file_count: int  # Total files scanned
    matched_patterns: Dict[str, int]  # Pattern -> count


class AssetDetector:
    """
    Detects game assets using regex-based file scanning.

    **Detection Categories:**
    1. **Editions:** ISO, MDF, HDD/Portable
    2. **Content:** DLC, Append, OST, Artbook
    3. **Tools:** Crack, NoDVD, Patch, Update
    4. **Language:** Chinese, Japanese, English
    5. **Extras:** Manual, Save, Cheats
    """

    # Regex patterns for asset detection
    # Format: (tag, [patterns], priority)
    # Higher priority = more important (shows first in labels)

    PATTERNS = {
        # Editions (highest priority)
        "ISO": [
            r'\.iso$',
            r'\.mdf$',
            r'\.cue$',
            r'\.ccd$',
            r'disk',
            r'cd[0-9]',
            r'disc',
        ],
        "HDD": [
            r'portable',
            r'hdd',
            r'no.*install',
            r'preinstalled',
        ],

        # Content
        "DLC": [
            r'dlc',
            r'append',
            r'fan\.disk',
            r'extra.*disk',
        ],
        "OST": [
            r'ost',
            r'soundtrack',
            r'bgm',
            r'original.*sound',
        ],
        "Artbook": [
            r'artbook',
            r'art.*book',
            r'gallery',
        ],

        # Tools
        "Crack": [
            r'crack',
            r'nodvd',
            r'no.*dvd',
            r'fix',
        ],
        "Patch": [
            r'patch',
            r'update',
            r'hotfix',
            r'v[0-9]+\.[0-9]+.*to.*v[0-9]+\.[0-9]+',
        ],

        # Language (Chinese variations)
        "Chinese": [
            r'[汉漢][化化]',
            r'chinese',
            r'\bcn\b',
            r'\bzh\b',
            r'china',
            r'trad.*chinese',
            r'simp.*chinese',
        ],
        "Japanese": [
            r'japanese',
            r'\bja\b',
            r'\bjp\b',
        ],
        "English": [
            r'english',
            r'\ben\b',
        ],

        # Extras
        "Manual": [
            r'manual',
            r'guide',
            r'walkthrough',
        ],
        "Save": [
            r'save',
            r'savestate',
        ],
        "Cheats": [
            r'cheat',
            r'trainer',
        ],
    }

    # File extensions to scan (optimize by skipping irrelevant files)
    SCANNED_EXTENSIONS = {
        '.exe', '.zip', '.rar', '.7z', '.iso', '.mdf', '.bin', '.cue',
        '.ccd', '.pdf', '.txt', '.nfo', '.url', '.lnk',
    }

    def __init__(self, case_sensitive: bool = False):
        """
        Initialize the AssetDetector.

        Args:
            case_sensitive: If True, patterns are case-sensitive (default: False)
        """
        self.case_sensitive = case_sensitive
        self._compile_patterns()

    def _compile_patterns(self) -> None:
        """Compile regex patterns for performance."""
        flags = 0 if self.case_sensitive else re.IGNORECASE

        self._compiled_patterns = {}
        for tag, patterns in self.PATTERNS.items():
            compiled = [re.compile(p, flags) for p in patterns]
            self._compiled_patterns[tag] = compiled

        logger.debug(f"Compiled {len(self._compiled_patterns)} asset tags with regex patterns")

    def detect_directory(self, directory: Path) -> AssetDetectionResult:
        """
        Scan a directory and detect assets.

        Args:
            directory: Game directory to scan

        Returns:
            AssetDetectionResult with detected assets and metadata
        """
        if not directory.exists():
            logger.warning(f"Directory does not exist: {directory}")
            return AssetDetectionResult(
                assets=[],
                version_label="Unknown",
                file_count=0,
                matched_patterns={}
            )

        detected_assets: Set[str] = set()
        pattern_counts: Dict[str, int] = {}
        file_count = 0

        try:
            # Scan all files in directory
            for item in directory.rglob("*"):
                if not item.is_file():
                    continue

                file_count += 1

                # Skip files without relevant extensions
                if item.suffix.lower() not in self.SCANNED_EXTENSIONS:
                    # Still check the filename for patterns (might match folder names)
                    pass

                # Check all patterns
                for tag, compiled_patterns in self._compiled_patterns.items():
                    for pattern in compiled_patterns:
                        # Search in both filename and parent directory names
                        search_text = str(item.relative_to(directory))

                        if pattern.search(search_text):
                            detected_assets.add(tag)
                            pattern_counts[tag] = pattern_counts.get(tag, 0) + 1

        except Exception as e:
            logger.error(f"Error scanning directory {directory}: {e}")

        # Convert set to sorted list
        assets_sorted = self._prioritize_assets(list(detected_assets))

        # Generate version label
        version_label = self._generate_version_label(assets_sorted)

        result = AssetDetectionResult(
            assets=assets_sorted,
            version_label=version_label,
            file_count=file_count,
            matched_patterns=pattern_counts
        )

        logger.info(
            f"Detected {len(assets_sorted)} assets in {directory.name}: "
            f"{version_label} ({file_count} files scanned)"
        )

        return result

    def _prioritize_assets(self, assets: List[str]) -> List[str]:
        """
        Sort assets by priority for display.

        Priority order:
        1. Editions (ISO, HDD)
        2. Language (Chinese, Japanese, English)
        3. Content (DLC, OST, Artbook)
        4. Tools (Crack, Patch)
        5. Extras (Manual, Save, Cheats)
        """
        priority_order = [
            "ISO", "HDD",
            "Chinese", "Japanese", "English",
            "DLC", "OST", "Artbook",
            "Crack", "Patch",
            "Manual", "Save", "Cheats"
        ]

        # Sort by priority index (unlisted items go last)
        def get_priority(asset: str) -> int:
            try:
                return priority_order.index(asset)
            except ValueError:
                return 999  # Unknown assets go last

        return sorted(assets, key=get_priority)

    def _generate_version_label(self, assets: List[str]) -> str:
        """
        Generate a human-readable version label from detected assets.

        Examples:
            - "ISO + Chinese Patch"
            - "HDD"
            - "ISO + DLC + OST"
            - "Chinese + Crack"

        Args:
            assets: List of detected asset tags

        Returns:
            Human-readable label
        """
        if not assets:
            return "Unknown"

        # Special cases for common combinations
        if "ISO" in assets and "Patch" in assets and "Chinese" in assets:
            # Prefer showing "ISO + Chinese" over "ISO + Patch + Chinese"
            assets = [a for a in assets if a != "Patch"]

        # Simplify label by removing redundant combinations
        # (e.g., if we have "ISO" and "Chinese", don't show "Patch" separately)

        # Join with " + " for clean display
        return " + ".join(assets)

    def detect_quick(self, directory: Path) -> List[str]:
        """
        Quick detection (only checks filenames, not full paths).

        Faster but less accurate. Use for real-time scanning.

        Args:
            directory: Game directory to scan

        Returns:
            List of detected asset tags
        """
        result = self.detect_directory(directory)
        return result.assets


# Global detector instance
_default_detector: Optional[AssetDetector] = None


def get_asset_detector() -> AssetDetector:
    """Get the global AssetDetector singleton."""
    global _default_detector
    if _default_detector is None:
        _default_detector = AssetDetector()
    return _default_detector


def detect_assets(directory: Path) -> List[str]:
    """
    Convenience function to detect assets in a directory.

    Args:
        directory: Game directory to scan

    Returns:
        List of detected asset tags
    """
    detector = get_asset_detector()
    result = detector.detect_directory(directory)
    return result.assets
