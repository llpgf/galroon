"""
Proposal Engine for Galgame Library Manager.

**PHASE 9.5: The Curator Workbench**

Provides read-only analysis of messy game folders and generates
a safe, reviewable organization plan.

Key Features:
- Deep scans source directory
- Categorizes files using AssetDetector and Standards
- Generates reviewable proposal (NO files moved yet)
- Handles conflicts and unknown files
- Tracks split archives

Usage:
    proposal = generate_proposal(
        source_path=Path("H:/MessyGames/Fate"),
        target_root=Path("D:/Games"),
        vndb_metadata={"developer": "Type-Moon", "year": "2004", ...}
    )

    # Review proposal
    for move in proposal.moves:
        print(f"{move.source} -> {move.target}")

    # Execute proposal (after user approval)
    execute_plan(proposal)
"""

import hashlib
import json
import logging
from pathlib import Path
from typing import Dict, List, Optional, Any, Set
from dataclasses import dataclass, field, asdict
from datetime import datetime
from enum import Enum

from .standards import (
    StandardDir,
    categorize_file,
    generate_standard_path,
    is_split_archive,
    get_archive_group_name,
    OrganizationStandard
)
from ..metadata.inventory import AssetDetector

logger = logging.getLogger(__name__)


class MoveStatus(Enum):
    """Status of a proposed file move."""
    SAFE = "safe"  # Automatically categorized
    UNRESOLVED = "unresolved"  # Needs user decision
    SKIP = "skip"  # Should be skipped (e.g., temp files)
    WARNING = "warning"  # Potential conflict


@dataclass
class FileMove:
    """
    Represents a single file move in the proposal.

    Attributes:
        source: Source file path (absolute)
        target: Target file path (absolute)
        status: MoveStatus enum
        category: StandardDir or "UNKNOWN"
        reason: Human-readable explanation
        size: File size in bytes
        checksum: MD5 checksum (for verification)
    """
    source: Path
    target: Path
    status: MoveStatus
    category: str  # StandardDir value or "UNKNOWN"
    reason: str
    size: int = 0
    checksum: str = ""

    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        return {
            "source": str(self.source),
            "target": str(self.target),
            "status": self.status.value,
            "category": self.category,
            "reason": self.reason,
            "size": self.size,
            "checksum": self.checksum
        }


@dataclass
class ArchiveGroup:
    """
    Represents a group of split archives that should stay together.

    Attributes:
        base_name: Base name without part numbers
        files: List of FileMove objects
        target_dir: Target directory (StandardDir name)
    """
    base_name: str
    files: List[FileMove]
    target_dir: str


@dataclass
class OrganizationProposal:
    """
    Complete organization proposal for review.

    Attributes:
        proposal_id: Unique ID for this proposal
        source_path: Original messy path
        target_structure: Complete target structure (from standards.py)
        vndb_metadata: VNDB metadata used for naming
        moves: List of all file moves
        categorized_moves: Dict of category -> list of moves
        archive_groups: List of split archive groups
        unresolved_files: Files needing user decision
        created_at: Timestamp
        total_size: Total size of all files (bytes)
        file_count: Total number of files
    """
    proposal_id: str
    source_path: Path
    target_structure: Dict[str, Path]
    vndb_metadata: Dict[str, Any]
    moves: List[FileMove] = field(default_factory=list)
    categorized_moves: Dict[str, List[FileMove]] = field(default_factory=dict)
    archive_groups: List[ArchiveGroup] = field(default_factory=list)
    unresolved_files: List[FileMove] = field(default_factory=list)
    created_at: str = field(default_factory=lambda: datetime.now().isoformat())
    total_size: int = 0
    file_count: int = 0

    def get_summary(self) -> Dict[str, Any]:
        """Get summary statistics for UI display."""
        categorized_counts = {
            category: len(moves)
            for category, moves in self.categorized_moves.items()
        }

        return {
            "proposal_id": self.proposal_id,
            "source_path": str(self.source_path),
            "target_base": str(self.target_structure["base"]),
            "file_count": self.file_count,
            "total_size_mb": round(self.total_size / (1024 * 1024), 2),
            "categorized_counts": categorized_counts,
            "unresolved_count": len(self.unresolved_files),
            "archive_groups": len(self.archive_groups),
            "created_at": self.created_at
        }


def generate_proposal(
    source_path: Path,
    target_root: Path,
    vndb_metadata: Dict[str, Any],
    asset_detector: Optional[AssetDetector] = None
) -> OrganizationProposal:
    """
    Generate a complete organization proposal for a game directory.

    **READ-ONLY OPERATION** - No files are moved.

    Args:
        source_path: Messy source directory to analyze
        target_root: Root library directory
        vndb_metadata: Metadata dict with keys:
            - developer: Developer/brand name
            - year: Release year (can be "2004" or "2004-01-30")
            - title: Game title
            - vndb_id: VNDB identifier (e.g., "v12345")
        asset_detector: Optional AssetDetector instance

    Returns:
        OrganizationProposal with complete plan
    """
    logger.info(f"Generating proposal for: {source_path}")

    # Create asset detector if not provided
    if asset_detector is None:
        asset_detector = AssetDetector()

    # Detect assets
    detection_result = asset_detector.detect_directory(source_path)
    detected_assets = detection_result.assets

    # Generate proposal ID
    proposal_id = hashlib.md5(
        f"{source_path}_{datetime.now().isoformat()}".encode()
    ).hexdigest()[:12]

    # Extract year from vndb_metadata (handle YYYY-MM-DD format)
    year = vndb_metadata.get("year", "Unknown")
    if "-" in str(year):
        year = str(year).split("-")[0]

    # Generate target structure
    target_structure = OrganizationStandard.generate_target_structure(
        source_path=source_path,
        library_root=target_root,
        developer=vndb_metadata.get("developer", "Unknown"),
        year=year,
        title=vndb_metadata.get("title", "Unknown"),
        vndb_id=vndb_metadata.get("vndb_id", "unknown")
    )

    # Initialize proposal
    proposal = OrganizationProposal(
        proposal_id=proposal_id,
        source_path=source_path,
        target_structure=target_structure,
        vndb_metadata=vndb_metadata
    )

    # Deep scan source directory
    all_files = list(source_path.rglob("*"))
    all_files = [f for f in all_files if f.is_file()]

    logger.info(f"Found {len(all_files)} files in source directory")

    # Group split archives
    archive_groups: Dict[str, List[Path]] = {}
    for file_path in all_files:
        if is_split_archive(file_path):
            group_name = get_archive_group_name(file_path)
            if group_name not in archive_groups:
                archive_groups[group_name] = []
            archive_groups[group_name].append(file_path)

    # Process each file
    processed_archives: Set[str] = set()

    for file_path in all_files:
        try:
            # Get file size
            file_size = file_path.stat().st_size
            proposal.total_size += file_size
            proposal.file_count += 1

            # Calculate checksum (for verification during execution)
            checksum = calculate_file_checksum(file_path)

            # Check if part of split archive group
            if is_split_archive(file_path):
                group_name = get_archive_group_name(file_path)
                if group_name in processed_archives:
                    # Already processed as part of group
                    continue

                # Process entire archive group
                group_files = archive_groups.get(group_name, [file_path])
                processed_archives.add(group_name)

                # Categorize based on first file
                category = categorize_file(file_path, detected_assets)
                if category is None:
                    category = "UNKNOWN"

                # Target path for archive group
                target_dir_name = category if isinstance(category, str) else (category.value if category else "Repository")
                target_dir = target_structure[target_dir_name]

                # Keep archive group together in target
                for i, archive_file in enumerate(group_files):
                    relative_name = archive_file.name
                    target_path = target_dir / relative_name

                    move = FileMove(
                        source=archive_file,
                        target=target_path,
                        status=MoveStatus.SAFE if category != "UNKNOWN" else MoveStatus.UNRESOLVED,
                        category=target_dir_name,
                        reason=f"Split archive group: {group_name}",
                        size=archive_file.stat().st_size,
                        checksum=calculate_file_checksum(archive_file)
                    )
                    proposal.moves.append(move)

                # Add to archive groups
                archive_group = ArchiveGroup(
                    base_name=group_name,
                    files=[m for m in proposal.moves if m.source in group_files],
                    target_dir=target_dir_name
                )
                proposal.archive_groups.append(archive_group)

                continue

            # Regular file (not part of split archive)
            category = categorize_file(file_path, detected_assets)

            # Determine target path
            if category:
                # Categorized file
                target_dir_name = category.value
                target_dir = target_structure[target_dir_name]
                status = MoveStatus.SAFE
            else:
                # Unknown file
                target_dir_name = "UNKNOWN"
                target_dir = target_structure["base"]  # Temporarily place at base
                status = MoveStatus.UNRESOLVED

            # Calculate relative path for target
            # Try to preserve some directory structure for Game/ folder
            if category == StandardDir.GAME:
                # Preserve relative path from source
                try:
                    relative_path = file_path.relative_to(source_path)
                except ValueError:
                    relative_path = file_path.name
                target_path = target_dir / relative_path
            else:
                # Flat structure for other categories
                target_path = target_dir / file_path.name

            # Create move
            move = FileMove(
                source=file_path,
                target=target_path,
                status=status,
                category=target_dir_name,
                reason=f"Categorized as {target_dir_name}" if category else "Unknown file type",
                size=file_size,
                checksum=checksum
            )
            proposal.moves.append(move)

        except Exception as e:
            logger.error(f"Error processing file {file_path}: {e}")

    # Categorize moves
    proposal.categorized_moves = {}
    for move in proposal.moves:
        if move.category not in proposal.categorized_moves:
            proposal.categorized_moves[move.category] = []
        proposal.categorized_moves[move.category].append(move)

    # Separate unresolved files
    if "UNKNOWN" in proposal.categorized_moves:
        proposal.unresolved_files = proposal.categorized_moves.pop("UNKNOWN")

    logger.info(
        f"Proposal generated: {proposal.file_count} files, "
        f"{len(proposal.unresolved_files)} unresolved, "
        f"{len(proposal.archive_groups)} archive groups"
    )

    return proposal


def calculate_file_checksum(file_path: Path, algorithm: str = "md5") -> str:
    """
    Calculate file checksum for verification.

    Args:
        file_path: File to checksum
        algorithm: Hash algorithm (default: md5)

    Returns:
        Hex checksum string
    """
    hash_func = hashlib.new(algorithm)

    try:
        with open(file_path, "rb") as f:
            # Read in chunks to handle large files
            for chunk in iter(lambda: f.read(8192), b""):
                hash_func.update(chunk)
        return hash_func.hexdigest()
    except Exception as e:
        logger.error(f"Error calculating checksum for {file_path}: {e}")
        return ""


def save_proposal(proposal: OrganizationProposal, output_path: Path) -> bool:
    """
    Save proposal to JSON file for review.

    Args:
        proposal: Proposal to save
        output_path: Output file path

    Returns:
        True if successful
    """
    try:
        proposal_dict = {
            "proposal_id": proposal.proposal_id,
            "source_path": str(proposal.source_path),
            "target_structure": {k: str(v) for k, v in proposal.target_structure.items()},
            "vndb_metadata": proposal.vndb_metadata,
            "moves": [move.to_dict() for move in proposal.moves],
            "archive_groups": [
                {
                    "base_name": g.base_name,
                    "files": [f.to_dict() for f in g.files],
                    "target_dir": g.target_dir
                }
                for g in proposal.archive_groups
            ],
            "unresolved_files": [f.to_dict() for f in proposal.unresolved_files],
            "summary": proposal.get_summary(),
            "created_at": proposal.created_at
        }

        output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump(proposal_dict, f, indent=2, ensure_ascii=False)

        logger.info(f"Proposal saved to: {output_path}")
        return True

    except Exception as e:
        logger.error(f"Error saving proposal: {e}")
        return False


def load_proposal(proposal_path: Path) -> Optional[OrganizationProposal]:
    """
    Load proposal from JSON file.

    Args:
        proposal_path: Path to proposal JSON file

    Returns:
        OrganizationProposal or None if error
    """
    try:
        with open(proposal_path, "r", encoding="utf-8") as f:
            data = json.load(f)

        # Reconstruct proposal
        proposal = OrganizationProposal(
            proposal_id=data["proposal_id"],
            source_path=Path(data["source_path"]),
            target_structure={k: Path(v) for k, v in data["target_structure"].items()},
            vndb_metadata=data["vndb_metadata"],
            created_at=data.get("created_at", datetime.now().isoformat())
        )

        # Reconstruct moves
        for move_data in data["moves"]:
            move = FileMove(
                source=Path(move_data["source"]),
                target=Path(move_data["target"]),
                status=MoveStatus(move_data["status"]),
                category=move_data["category"],
                reason=move_data["reason"],
                size=move_data.get("size", 0),
                checksum=move_data.get("checksum", "")
            )
            proposal.moves.append(move)

        # Reconstruct categorized moves
        for move in proposal.moves:
            if move.category not in proposal.categorized_moves:
                proposal.categorized_moves[move.category] = []
            proposal.categorized_moves[move.category].append(move)

        # Reconstruct archive groups
        for group_data in data.get("archive_groups", []):
            group_files = []
            for file_data in group_data["files"]:
                file_move = FileMove(
                    source=Path(file_data["source"]),
                    target=Path(file_data["target"]),
                    status=MoveStatus(file_data["status"]),
                    category=file_data["category"],
                    reason=file_data["reason"],
                    size=file_data.get("size", 0),
                    checksum=file_data.get("checksum", "")
                )
                group_files.append(file_move)

            group = ArchiveGroup(
                base_name=group_data["base_name"],
                files=group_files,
                target_dir=group_data["target_dir"]
            )
            proposal.archive_groups.append(group)

        # Reconstruct unresolved files
        for file_data in data.get("unresolved_files", []):
            file_move = FileMove(
                source=Path(file_data["source"]),
                target=Path(file_data["target"]),
                status=MoveStatus(file_data["status"]),
                category=file_data["category"],
                reason=file_data["reason"],
                size=file_data.get("size", 0),
                checksum=file_data.get("checksum", "")
            )
            proposal.unresolved_files.append(file_move)

        # Update stats
        proposal.file_count = len(proposal.moves)
        proposal.total_size = sum(m.size for m in proposal.moves)

        logger.info(f"Proposal loaded from: {proposal_path}")
        return proposal

    except Exception as e:
        logger.error(f"Error loading proposal: {e}")
        return None
