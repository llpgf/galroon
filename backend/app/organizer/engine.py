"""
Reorganization Engine - The Crown Engine
Orchestrates physical file layout strategies.

Modes:
- A (Virtual): Writes metadata.json only. No moves.
- B (Museum): Atomically moves content to structured path. leaves symlinks.
"""

import logging
from pathlib import Path
from typing import Dict, Any, List, Optional
from dataclasses import dataclass
from datetime import datetime

from ..core.database import Database
from .safety import SafetyOps
from .sidecar import SidecarGenerator

logger = logging.getLogger(__name__)

@dataclass
class PreviewResult:
    mode: str
    original_path: str
    new_path: str
    actions: List[str]  # Description of actions
    warnings: List[str]

class ReorganizationEngine:
    def __init__(self, db: Database):
        self.db = db

    def _get_game_data(self, canonical_id: str) -> Optional[Dict[str, Any]]:
        row = self.db.conn.execute(
             "SELECT * FROM canonical_games WHERE id = ?", (canonical_id,)
        ).fetchone()
        if row:
            return dict(row)
        return None

    def _get_local_instances(self, canonical_id: str) -> List[Dict[str, Any]]:
        rows = self.db.conn.execute(
            "SELECT * FROM local_instances WHERE canonical_id = ?", (canonical_id,)
        ).fetchall()
        return [dict(r) for r in rows]

    def _calculate_target_path(self, game: Dict[str, Any], root: Path) -> Path:
        """
        Calculate target path: Root / Year / Developer / Title
        Example: D:/Galgame/2004/Type-Moon/Fate Stay Night
        """
        meta = game.get('metadata_snapshot', {}) or {}
        # Safely parse date
        year = "Unknown Year"
        rd = meta.get('release_date')
        if rd and len(rd) >= 4:
            year = rd[:4]

        dev = meta.get('developer', 'Unknown Developer')
        # Sanitize for filesystem
        safe_dev = "".join([c for c in dev if c.isalnum() or c in (' ', '-', '_')]).strip()
        safe_title = "".join([c for c in game['display_title'] if c.isalnum() or c in (' ', '-', '_')]).strip()
        
        return root / year / safe_dev / safe_title

    def dry_run(self, canonical_id: str, mode: str, library_root_override: str = None) -> PreviewResult:
        game = self._get_game_data(canonical_id)
        if not game:
            raise ValueError(f"Game {canonical_id} not found")

        instances = self._get_local_instances(canonical_id)
        if not instances:
             raise ValueError("No local files found for this game")
        
        # Assume primary instance for move
        instance = instances[0] 
        source_path = Path(instance['folder_path'])
        
        # Use existing parent of source as default root if no override
        # BUT for Reorg, we typically want a standardized root.
        # For prototype, let's assume we stay in the same Volume/Root if possible, 
        # or use the "First Library Root" settings.
        # For simplicity in Sprint 10.5, let's use source_path.parent.parent as 'Root' assumption 
        # or require config.
        # Let's pivot: Just use source_path.parent as the 'Library Root' context.
        library_root = Path(library_root_override) if library_root_override else source_path.parent

        target_path = source_path # Default for Mode A
        actions = []
        warnings = []

        if mode == "A":
            actions.append("Generate metadata.json in current folder")
            actions.append("Generate game.nfo in current folder")
            actions.append("No files will be moved")
        
        elif mode == "B":
            target_path = self._calculate_target_path(game, library_root)
            if target_path != source_path:
                actions.append(f"Move folder to: {target_path}")
                actions.append(f"Create Symlink at: {source_path}")
            else:
                actions.append("Path structure already correct matches Canonical Standard")
                actions.append("No move necessary")
            
            actions.append("Generate metadata.json")
            actions.append("Generate game.nfo")

            # Check permissions
            if not SafetyOps.ensure_safe_path(library_root, target_path):
                warnings.append("Target path is outside safety bounds!")
        
        return PreviewResult(
            mode=mode,
            original_path=str(source_path),
            new_path=str(target_path),
            actions=actions,
            warnings=warnings
        )

    def execute(self, canonical_id: str, mode: str, library_root_override: str = None) -> Dict[str, Any]:
        """
        Execute the reorganization
        """
        preview = self.dry_run(canonical_id, mode, library_root_override)
        game = self._get_game_data(canonical_id)
        
        source = Path(preview.original_path)
        dest = Path(preview.new_path)
        
        result_log = []

        if mode == "B":
            if source != dest:
                # 1. Atomic Move
                logger.info(f"Moving {source} -> {dest}")
                if not SafetyOps.atomic_move(source, dest):
                    raise RuntimeError("Move failed during execution")
                result_log.append("Files moved successfully")

                # 2. Create Symlink
                if SafetyOps.create_symlink(dest, source, is_dir=True):
                    result_log.append("Symlink created")
                else:
                    result_log.append("Symlink creation failed (non-critical)")

                # Update database path?? 
                # Yes, local_instance needs update? 
                # OR we keep local_instance pointing to Symlink?
                # "The Museum" implies the new path is now the Real One.
                # So we should update local_instances.
                self.db.conn.execute(
                    "UPDATE local_instances SET folder_path = ? WHERE canonical_id = ?",
                    (str(dest), canonical_id)
                )
                self.db.conn.commit()

        # Generate Sidecars (Both A and B)
        # For B, we write to dest. For A, dest == source.
        target_dir = dest if mode == "B" else source
        
        # Files structure for metadata.json 
        # Scan dir to get simple file list
        files_list = []
        if target_dir.exists():
             for f in target_dir.rglob("*"):
                 if f.is_file():
                     files_list.append({"rel_path": str(f.relative_to(target_dir)), "role": "unknown"})

        SidecarGenerator.generate_metadata_json(game, files_list, target_dir)
        SidecarGenerator.generate_nfo(game, target_dir)
        result_log.append("Metadata sidecars generated")

        return {
            "success": True,
            "mode": mode,
            "final_path": str(target_dir),
            "log": result_log
        }
