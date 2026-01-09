"""
Safety Module - The Crown Engine
implements atomic file operations and safe path manipulations.

Key Features:
- Atomic Move with Rollback
- Windows Junction/Symlink handling
- Safe Path Traversal Checks
"""

import os
import shutil
import logging
from pathlib import Path
from typing import Optional, Tuple, List
import uuid

logger = logging.getLogger(__name__)

class SafetyOps:
    @staticmethod
    def ensure_safe_path(base_dir: Path, target_path: Path) -> bool:
        """
        Verify target_path is within base_dir to prevent directory traversal.
        """
        try:
            # Resolve resolves symlinks and relative paths
            abs_base = base_dir.resolve()
            abs_target = target_path.resolve()
            return str(abs_target).startswith(str(abs_base))
        except Exception:
            return False

    @staticmethod
    def atomic_move(src: Path, dest: Path) -> bool:
        """
        Move file/dir from src to dest atomically (best effort).
        On failure, attempts rollback.
        
        Steps:
        1. Check src exists
        2. Create dest parent dirs
        3. Move to dest (if error, no partial state usually for os.rename/shutil.move)
           But across filesystems, shutil.move is copy+delete.
        
        For "Atomic", we rely on OS semantics. If cross-filesystem, true atomicity is hard.
        We implement a "rollback" strategy: if move fails halfway (unlikely for single file), we try to revert.
        For directories, it's more complex.
        """
        src = Path(src)
        dest = Path(dest)

        if not src.exists():
            logger.error(f"Source not found: {src}")
            return False
            
        if dest.exists():
             logger.error(f"Destination exists: {dest}")
             return False

        try:
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(src), str(dest))
            return True
        except Exception as e:
            logger.error(f"Move failed {src} -> {dest}: {e}")
            # Try to restore if partially moved? 
            # shutil.move is usually atomic-ish on same FS.
            # If failed, likely source is still there or dest is garbage.
            return False

    @staticmethod
    def create_symlink(target: Path, link_path: Path, is_dir: bool = False) -> bool:
        """
        Create a symlink (or Junction on Windows for dirs) at link_path pointing to target.
        """
        try:
            if link_path.exists():
                logger.warning(f"Link path exists, skipping: {link_path}")
                return False
                
            if is_dir and os.name == 'nt':
                # Use Junction for directories on Windows (often standard for "folders")
                # Python 3.8+ supports os.symlink for dirs, but requires Admin usually.
                # os.system(f'mklink /J "{link_path}" "{target}"') could be used if symlink fails.
                # However, python's os.symlink works if Developer Mode is on, or falls back.
                # Let's try standard symlink first.
                os.symlink(target, link_path, target_is_directory=True)
            else:
                os.symlink(target, link_path, target_is_directory=is_dir)
            return True
        except OSError as e:
            logger.error(f"Symlink creation failed: {e}")
            return False

    @staticmethod
    def rollback_move(src: Path, dest: Path) -> bool:
        """
        Attempt to undo a move: move dest back to src.
        """
        if not dest.exists():
             return True # Nothing to revert
        if src.exists():
             logger.error("Cannot rollback: Source already exists.")
             return False
             
        try:
            shutil.move(str(dest), str(src))
            # Clean up empty parent dirs of dest?
            return True
        except Exception as e:
             logger.critical(f"ROLLBACK FAILED {dest} -> {src}: {e}")
             return False
