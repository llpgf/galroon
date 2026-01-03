"""
Organization Standards for Galgame Library Manager.

**PHASE 9.5: The Curator Workbench**

Defines the target "Scene Standard" folder structure for organizing
messy game folders into a clean, predictable hierarchy.

Standard Structure:
    {Library_Root}/{Developer}/{Year} {Title} [{VNDB_ID}]/
        Game/           <- Extracted game files, executables
        Repository/     <- ISOs, installers, archives (ZIP/RAR), HDD folders
        Patch_Work/     <- Patches, updates, cracks, translations
        Extras/         <- OSTs, artbooks, manuals, saves
        Metadata/       <- metadata.json and cached images

This module provides:
1. Directory structure definitions
2. File categorization rules
3. Path generation utilities
4. Validation helpers
"""

import re
import logging
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass
from enum import Enum

logger = logging.getLogger(__name__)


class StandardDir(Enum):
    """
    Standard subdirectories in the organized structure.

    Each directory has a specific purpose and matching rules.
    """
    GAME = "Game"
    REPOSITORY = "Repository"
    PATCH_WORK = "Patch_Work"
    EXTRAS = "Extras"
    METADATA = "Metadata"


@dataclass
class DirRule:
    """
    Rule for categorizing files into standard directories.

    Attributes:
        dir: StandardDir enum value
        description: Human-readable description
        patterns: Regex patterns to match files
        extensions: File extensions to match
        asset_tags: AssetDetector tags that belong here
        requires_extraction: Whether files here are typically archives
    """
    dir: StandardDir
    description: str
    patterns: List[str]
    extensions: List[str]
    asset_tags: List[str]
    requires_extraction: bool = False


# Standard directory rules
DIR_RULES: Dict[StandardDir, DirRule] = {
    StandardDir.GAME: DirRule(
        dir=StandardDir.GAME,
        description="Extracted game files and executables",
        patterns=[
            r'\.exe$',  # Executables
            r'\.dll$',  # DLL libraries
            r'\.xp3$',  # Kirikiri data files
            r'\.dat$',  # Data files
        ],
        extensions=[".exe", ".dll", ".xp3", ".dat", ".pack"],
        asset_tags=["HDD", "Portable"],
        requires_extraction=False
    ),

    StandardDir.REPOSITORY: DirRule(
        dir=StandardDir.REPOSITORY,
        description="ISOs, installers, and archives (original distribution format)",
        patterns=[
            r'\.iso$',
            r'\.mdf$',
            r'\.cue$',
            r'\.ccd$',
            r'setup\.exe$',
            r'install\.exe$',
        ],
        extensions=[".iso", ".mdf", ".cue", ".bin", ".zip", ".rar", ".7z"],
        asset_tags=["ISO"],
        requires_extraction=True
    ),

    StandardDir.PATCH_WORK: DirRule(
        dir=StandardDir.PATCH_WORK,
        description="Patches, cracks, translations, and modifications",
        patterns=[
            r'patch',
            r'crack',
            r'nodvd',
            r'no.*dvd',
            r'update',
            r'hotfix',
            r'[汉漢][化化]',
            r'chinese',
            r'decensor',
        ],
        extensions=[".exe", ".zip", ".rar", ".7z"],
        asset_tags=["Patch", "Crack", "Chinese", "Update"],
        requires_extraction=False
    ),

    StandardDir.EXTRAS: DirRule(
        dir=StandardDir.EXTRAS,
        description="OSTs, artbooks, manuals, and bonus content",
        patterns=[
            r'ost',
            r'soundtrack',
            r'bgm',
            r'artbook',
            r'art.*book',
            r'gallery',
            r'manual',
            r'guide',
            r'walkthrough',
            r'save',
            r'savestate',
        ],
        extensions=[".mp3", ".flac", ".pdf", ".zip", ".rar"],
        asset_tags=["OST", "Artbook", "Manual", "Save"],
        requires_extraction=False
    ),

    StandardDir.METADATA: DirRule(
        dir=StandardDir.METADATA,
        description="System metadata and cached images",
        patterns=[
            r'metadata\.json$',
            r'folder\.jpg$',
            r'cover\.jpg$',
            r'background\.jpg$',
            r'\.png$',
            r'\.jpg$',
        ],
        extensions=[".json", ".jpg", ".png", ".webp"],
        asset_tags=[],
        requires_extraction=False
    ),
}


def categorize_file(file_path: Path, detected_assets: List[str]) -> Optional[StandardDir]:
    """
    Categorize a file into a standard directory based on its path and detected assets.

    Args:
        file_path: Path to the file
        detected_assets: List of asset tags from AssetDetector

    Returns:
        StandardDir if categorized, None if unknown
    """
    filename = file_path.name.lower()
    parent_name = file_path.parent.name.lower()
    full_path = str(file_path).lower()

    # Check asset tags first (highest priority)
    for dir_rule in DIR_RULES.values():
        for asset_tag in dir_rule.asset_tags:
            if asset_tag in detected_assets:
                # Special case: ISO with Chinese patch goes to Repository
                if asset_tag == "ISO":
                    return StandardDir.REPOSITORY
                # Other asset tags match their directory
                if asset_tag in ["Patch", "Crack", "Chinese", "Update"]:
                    return StandardDir.PATCH_WORK
                if asset_tag in ["OST", "Artbook", "Manual", "Save"]:
                    return StandardDir.EXTRAS

    # Check file extension and path patterns
    for dir_rule in DIR_RULES.values():
        # Check extensions
        if file_path.suffix.lower() in dir_rule.extensions:
            # Special case: metadata files
            if dir_rule.dir == StandardDir.METADATA:
                if filename in ["metadata.json", "folder.jpg", "cover.jpg"]:
                    return StandardDir.METADATA
                # Image files in root are metadata
                if file_path.suffix.lower() in [".jpg", ".png", ".webp"]:
                    return StandardDir.METADATA
            # Special case: executables in setup/installer folders
            elif dir_rule.dir == StandardDir.REPOSITORY:
                if any(x in parent_name for x in ["setup", "install", "installer"]):
                    return StandardDir.REPOSITORY
            # Other extensions
            else:
                return dir_rule.dir

        # Check regex patterns
        for pattern in dir_rule.patterns:
            if re.search(pattern, full_path, re.IGNORECASE):
                return dir_rule.dir

    # Unknown file
    return None


def generate_standard_path(
    library_root: Path,
    developer: str,
    year: str,
    title: str,
    vndb_id: str,
    sub_dir: Optional[StandardDir] = None
) -> Path:
    """
    Generate a standardized path following the Scene Standard.

    Format: {Library_Root}/{Developer}/{Year} {Title} [{VNDB_ID}]/[SubDir]

    Args:
        library_root: Root library directory
        developer: Developer/brand name (sanitized)
        year: Release year
        title: Game title (sanitized)
        vndb_id: VNDB identifier (e.g., "v12345")
        sub_dir: Optional subdirectory (StandardDir enum)

    Returns:
        Standardized path
    """
    # Sanitize components
    developer = sanitize_path_component(developer)
    title = sanitize_path_component(title)

    # Build base path: {Developer}/{Year} {Title} [{VNDB_ID}]
    base_name = f"{year} {title} [{vndb_id}]"
    base_path = library_root / developer / base_name

    # Add subdirectory if specified
    if sub_dir:
        return base_path / sub_dir.value

    return base_path


def sanitize_path_component(component: str) -> str:
    """
    Sanitize a path component by removing invalid characters.

    Args:
        component: Raw string (developer, title, etc.)

    Returns:
        Sanitized string safe for file paths
    """
    # Remove invalid characters: < > : " / \ | ? *
    sanitized = re.sub(r'[<>:"/\\|?*]', '', component)

    # Replace multiple spaces with single space
    sanitized = re.sub(r'\s+', ' ', sanitized)

    # Strip leading/trailing spaces and dots
    sanitized = sanitized.strip('. ')

    # Limit length (Windows has 260 char path limit)
    if len(sanitized) > 100:
        sanitized = sanitized[:97] + "..."

    return sanitized or "Unknown"


def get_standard_subdirs() -> List[StandardDir]:
    """
    Get list of all standard subdirectories.

    Returns:
        List of StandardDir enums
    """
    return list(StandardDir)


def validate_standard_structure(base_path: Path) -> Dict[StandardDir, bool]:
    """
    Validate that a directory follows the standard structure.

    Args:
        base_path: Base path to check (should be {Year} {Title} [{VNDB_ID}])

    Returns:
        Dictionary mapping StandardDir -> exists (bool)
    """
    result = {}

    for std_dir in StandardDir:
        dir_path = base_path / std_dir.value
        result[std_dir] = dir_path.exists() and dir_path.is_dir()

    return result


def is_split_archive(file_path: Path) -> bool:
    """
    Check if a file is part of a split archive (part1.rar, part2.rar, etc.).

    Args:
        file_path: File to check

    Returns:
        True if file appears to be part of a split archive
    """
    filename = file_path.name.lower()

    # Common split archive patterns
    split_patterns = [
        r'\.part\d+\.rar$',
        r'\.r\d+$',
        r'\.rar$',
        r'\.7z\.\d+$',
        r'\.zip\.\d+$',
    ]

    for pattern in split_patterns:
        if re.search(pattern, filename):
            return True

    # Check for numeric suffixes like .001, .002
    if re.search(r'\.\d{3}$', filename):
        return True

    return False


def get_archive_group_name(file_path: Path) -> str:
    """
    Get the base name for a group of split archives.

    Example:
        game.part1.rar -> game
        game.r01 -> game
        game.001 -> game

    Args:
        file_path: Archive file path

    Returns:
        Base name without part numbers
    """
    filename = file_path.name

    # Remove split archive suffixes
    base = re.sub(r'\.part\d+\.(rar|zip|7z)$', r'', filename, flags=re.IGNORECASE)
    base = re.sub(r'\.(r|z)\d+$', r'', base, flags=re.IGNORECASE)
    base = re.sub(r'\.(rar|zip|7z)\.\d+$', r'', base, flags=re.IGNORECASE)
    base = re.sub(r'\.\d{3}$', r'', base)

    return base


class OrganizationStandard:
    """
    Main interface for organization standards.

    Provides high-level methods for working with the standard structure.
    """

    @staticmethod
    def categorize_files(files: List[Path], detected_assets: List[str]) -> Dict[StandardDir, List[Path]]:
        """
        Categorize multiple files into standard directories.

        Args:
            files: List of file paths
            detected_assets: Asset tags from AssetDetector

        Returns:
            Dictionary mapping StandardDir -> list of files
        """
        categorized = {std_dir: [] for std_dir in StandardDir}
        categorized["UNKNOWN"] = []

        for file_path in files:
            if not file_path.is_file():
                continue

            std_dir = categorize_file(file_path, detected_assets)
            if std_dir:
                categorized[std_dir].append(file_path)
            else:
                categorized["UNKNOWN"].append(file_path)

        return categorized

    @staticmethod
    def generate_target_structure(
        source_path: Path,
        library_root: Path,
        developer: str,
        year: str,
        title: str,
        vndb_id: str
    ) -> Dict[str, Path]:
        """
        Generate complete target structure for a game.

        Args:
            source_path: Original messy path
            library_root: Target library root
            developer: Developer name
            year: Release year
            title: Game title
            vndb_id: VNDB identifier

        Returns:
            Dictionary with paths for all standard directories
        """
        base_path = generate_standard_path(
            library_root=library_root,
            developer=developer,
            year=year,
            title=title,
            vndb_id=vndb_id
        )

        structure = {
            "base": base_path,
            "source": source_path,
        }

        for std_dir in StandardDir:
            structure[std_dir.value] = base_path / std_dir.value

        return structure

    @staticmethod
    def detect_main_executable(files: List[Path]) -> Optional[Path]:
        """
        Detect the main game executable from a list of files.

        Heuristics:
        1. Prefer .exe files in root or common game folders
        2. Avoid setup/installer executables
        3. Prefer names matching "game", "startup", or the parent folder name

        Args:
            files: List of file paths

        Returns:
            Path to main executable or None
        """
        executables = [f for f in files if f.suffix.lower() == ".exe"]

        if not executables:
            return None

        # Filter out installers/setup
        game_exes = []
        for exe in executables:
            name_lower = exe.name.lower()
            if any(x in name_lower for x in ["setup", "install", "unins", "patch"]):
                continue
            game_exes.append(exe)

        if not game_exes:
            return None

        # Prefer executables with certain names
        preferred_names = ["game", "startup", "start", "launcher"]
        for exe in game_exes:
            for pref in preferred_names:
                if pref in exe.name.lower():
                    return exe

        # Otherwise return the first game exe
        return game_exes[0]
