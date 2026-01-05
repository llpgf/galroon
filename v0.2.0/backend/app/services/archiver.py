"""
Archive Service for Galgame Library Manager.

**THE UTILITY BELT: Archive Extraction**

Provides manual archive extraction helper:
- Extract archives (RAR, 7Z, ZIP) to target directory
- Multi-part archive support (part1.rar, part2.rar, ...)
- Background task execution with progress reporting
- 7-Zip (preferred) or patool fallback

Core Philosophy: Helper, not Manager.
User selects source → User selects target → App extracts → Done.
"""

import logging
import subprocess
import threading
import time
from pathlib import Path
from typing import Dict, Any, Optional, Callable
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum

logger = logging.getLogger(__name__)


class TaskStatus(str, Enum):
    """Status of extraction task."""
    PENDING = "pending"
    RUNNING = "running"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


@dataclass
class ExtractionTask:
    """
    Background extraction task.

    Attributes:
        task_id: Unique task identifier
        source_path: Archive file path
        target_dir: Extraction target directory
        status: Current task status
        progress: Progress percentage (0-100)
        current_file: File being extracted (if available)
        total_files: Total number of files to extract
        error: Error message if failed
        created_at: Task creation timestamp
        started_at: Task start timestamp
        completed_at: Task completion timestamp
    """
    task_id: str
    source_path: str
    target_dir: str
    status: TaskStatus = TaskStatus.PENDING
    progress: int = 0
    current_file: str = ""
    total_files: int = 0
    error: Optional[str] = None
    created_at: str = field(default_factory=lambda: datetime.now().isoformat())
    started_at: Optional[str] = None
    completed_at: Optional[str] = None


class ArchiveService:
    """
    Manual archive extraction helper.

    Supports:
    - 7-Zip (7z) for RAR, 7Z, ZIP, ISO, etc. (preferred)
    - patool library as fallback (python-native)
    - Multi-part archives (automatic detection)
    - Background execution with progress tracking
    """

    # 7-Zip executable names
    SEVEN_ZIP_EXES = ["7z", "7za", "7zr"]

    def __init__(self):
        """Initialize ArchiveService."""
        self.seven_zip_path = self._find_7zip()
        self.use_patool = self.seven_zip_path is None

        if self.seven_zip_path:
            logger.info(f"Using 7-Zip: {self.seven_zip_path}")
        else:
            logger.warning("7-Zip not found, will use patool (limited format support)")

        # Background tasks storage
        self.tasks: Dict[str, ExtractionTask] = {}
        self._task_lock = threading.Lock()

    def _find_7zip(self) -> Optional[str]:
        """
        Find 7-Zip executable in system PATH.

        Returns:
            Path to 7-Zip executable or None if not found
        """
        for exe_name in self.SEVEN_ZIP_EXES:
            try:
                # Try 'where' command on Windows
                result = subprocess.run(
                    ["where", exe_name],
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                if result.returncode == 0:
                    path = result.stdout.strip().split('\n')[0]
                    logger.debug(f"Found 7-Zip via 'where': {path}")
                    return path

            except (subprocess.CalledProcessError, FileNotFoundError):
                pass

            try:
                # Try 'which' command on Unix
                result = subprocess.run(
                    ["which", exe_name],
                    capture_output=True,
                    text=True,
                    timeout=5
                )
                if result.returncode == 0:
                    path = result.stdout.strip()
                    logger.debug(f"Found 7-Zip via 'which': {path}")
                    return path

            except (subprocess.CalledProcessError, FileNotFoundError):
                pass

        logger.warning("7-Zip not found in PATH")
        return None

    def _detect_multipart_files(self, archive_path: Path) -> list[Path]:
        """
        Detect all parts of a multi-part archive.

        Patterns:
        - archive.part1.rar, archive.part2.rar, ...
        - archive.rar, archive.r00, archive.r01, ...
        - archive.7z.001, archive.7z.002, ...

        Args:
            archive_path: First part of archive

        Returns:
            List of all archive parts in order
        """
        parts = [archive_path]
        base_name = archive_path.name
        parent_dir = archive_path.parent

        # Pattern 1: .partN.rar
        if ".part1." in base_name.lower():
            # Find all parts: .part1.rar, .part2.rar, ...
            prefix = base_name.lower().replace(".part1.", ".part")
            prefix_stem = prefix.rsplit(".", 1)[0]  # Remove .rar

            for sibling in parent_dir.glob(f"{prefix_stem}.*"):
                if sibling.is_file():
                    # Check if it matches pattern
                    if ".part" in sibling.name.lower():
                        parts.append(sibling)

        # Pattern 2: .rar, .r00, .r01, ...
        elif base_name.lower().endswith(".rar"):
            stem = base_name[:-4]  # Remove .rar

            # Look for .r00, .r01, ...
            for sibling in parent_dir.glob(f"{stem}.r*"):
                if sibling.is_file() and sibling != archive_path:
                    # Check if it's a numbered part (.r00, .r01, etc.)
                    suffix = sibling.name[len(stem):]
                    if suffix.startswith(".r") and suffix[2:].isdigit():
                        parts.append(sibling)

        # Pattern 3: .7z.001, .7z.002, ...
        elif ".7z." in base_name.lower() and base_name.lower().endswith((".001", ".000")):
            stem = base_name.rsplit(".", 1)[0]  # Remove .001

            for sibling in parent_dir.glob(f"{stem}.*"):
                if sibling.is_file():
                    suffix = sibling.name[len(stem)+1:]
                    if suffix.isdigit():
                        parts.append(sibling)

        # Sort parts naturally
        def natural_key(path_obj):
            """Natural sorting key for multi-part files."""
            name = path_obj.name.lower()

            # Extract numbers for sorting
            import re
            numbers = re.findall(r'\d+', name)
            return [int(n) for n in numbers] if numbers else [0]

        parts.sort(key=natural_key)

        logger.info(f"Detected {len(parts)} archive parts")
        return parts

    def _extract_with_7zip(
        self,
        archive_path: Path,
        target_dir: Path,
        progress_callback: Optional[Callable[[int, str], None]] = None
    ) -> Dict[str, Any]:
        """
        Extract archive using 7-Zip.

        Args:
            archive_path: Path to archive file
            target_dir: Target extraction directory
            progress_callback: Optional callback(progress_percent, current_file)

        Returns:
            Dict with success status and details
        """
        try:
            target_dir.mkdir(parents=True, exist_ok=True)

            # Detect multi-part archives
            archive_parts = self._detect_multipart_files(archive_path)

            if len(archive_parts) > 1:
                logger.info(f"Extracting multi-part archive ({len(archive_parts)} parts)")

                # Extract all parts (7-Zip handles this automatically)
                # We just need to extract the first part
                result = subprocess.run(
                    [
                        self.seven_zip_path,
                        "x",  # eXtract with full paths
                        str(archive_parts[0].resolve()),
                        f"-o{target_dir.resolve()}",
                        "-y"  # Yes to all prompts
                    ],
                    capture_output=True,
                    text=True,
                    timeout=600  # 10 minute timeout
                )

                if result.returncode == 0:
                    return {
                        "success": True,
                        "method": "7zip",
                        "parts": len(archive_parts),
                        "message": f"Extracted {len(archive_parts)} parts successfully"
                    }
                else:
                    error = result.stderr or result.stdout
                    return {
                        "success": False,
                        "method": "7zip",
                        "error": f"7-Zip error: {error}"
                    }

            else:
                # Single archive
                logger.info(f"Extracting single archive: {archive_path.name}")

                result = subprocess.run(
                    [
                        self.seven_zip_path,
                        "x",
                        str(archive_path.resolve()),
                        f"-o{target_dir.resolve()}",
                        "-y"
                    ],
                    capture_output=True,
                    text=True,
                    timeout=600
                )

                if result.returncode == 0:
                    return {
                        "success": True,
                        "method": "7zip",
                        "message": f"Extracted successfully"
                    }
                else:
                    error = result.stderr or result.stdout
                    return {
                        "success": False,
                        "method": "7zip",
                        "error": f"7-Zip error: {error}"
                    }

        except subprocess.TimeoutExpired:
            return {
                "success": False,
                "method": "7zip",
                "error": "Extraction timed out (10 minutes)"
            }
        except Exception as e:
            return {
                "success": False,
                "method": "7zip",
                "error": f"Extraction failed: {str(e)}"
            }

    def _extract_with_patool(
        self,
        archive_path: Path,
        target_dir: Path,
        progress_callback: Optional[Callable[[int, str], None]] = None
    ) -> Dict[str, Any]:
        """
        Extract archive using patool library (fallback).

        Args:
            archive_path: Path to archive file
            target_dir: Target extraction directory
            progress_callback: Optional callback(progress_percent, current_file)

        Returns:
            Dict with success status and details
        """
        try:
            import patoolib

            target_dir.mkdir(parents=True, exist_ok=True)

            logger.info(f"Extracting with patool: {archive_path.name}")

            # Extract using patool
            patoolib.extract_archive(
                str(archive_path.resolve()),
                outdir=str(target_dir.resolve())
            )

            return {
                "success": True,
                "method": "patool",
                "message": "Extracted successfully"
            }

        except ImportError:
            return {
                "success": False,
                "method": "patool",
                "error": "patool library not installed. Install with: pip install patoolib"
            }
        except Exception as e:
            return {
                "success": False,
                "method": "patool",
                "error": f"Extraction failed: {str(e)}"
            }

    def extract_archive(
        self,
        source_path: str,
        target_dir: str,
        background: bool = True
    ) -> Dict[str, Any]:
        """
        Extract archive to target directory.

        Args:
            source_path: Path to archive file
            target_dir: Target extraction directory
            background: If True, run in background thread (default)

        Returns:
            Dict with task_id (if background) or result (if sync)
        """
        source = Path(source_path)
        target = Path(target_dir)

        # Validate source
        if not source.exists():
            return {
                "success": False,
                "error": f"Archive does not exist: {source_path}"
            }

        # If background execution
        if background:
            # Create task
            task_id = f"extract_{int(time.time() * 1000)}"
            task = ExtractionTask(
                task_id=task_id,
                source_path=str(source),
                target_dir=str(target)
            )

            with self._task_lock:
                self.tasks[task_id] = task

            # Start background thread
            thread = threading.Thread(
                target=self._extract_background,
                args=(task,),
                daemon=True
            )
            thread.start()

            logger.info(f"Started extraction task: {task_id}")

            return {
                "success": True,
                "task_id": task_id,
                "message": "Extraction started in background"
            }

        else:
            # Synchronous extraction
            result = self._extract_sync(source, target)
            return result

    def _extract_sync(
        self,
        source: Path,
        target: Path
    ) -> Dict[str, Any]:
        """Perform synchronous extraction."""
        # Try 7-Zip first
        if self.seven_zip_path:
            result = self._extract_with_7zip(source, target)
            if result["success"]:
                return result

        # Fallback to patool
        logger.info("Falling back to patool")
        result = self._extract_with_patool(source, target)

        return result

    def _extract_background(self, task: ExtractionTask):
        """Perform background extraction with progress updates."""
        try:
            # Update status
            task.status = TaskStatus.RUNNING
            task.started_at = datetime.now().isoformat()

            source = Path(task.source_path)
            target = Path(task.target_dir)

            # Perform extraction
            result = self._extract_sync(source, target)

            # Update task with result
            if result["success"]:
                task.status = TaskStatus.COMPLETED
                task.progress = 100
            else:
                task.status = TaskStatus.FAILED
                task.error = result.get("error", "Unknown error")

            task.completed_at = datetime.now().isoformat()

            logger.info(f"Extraction task {task.task_id} completed: {task.status}")

        except Exception as e:
            logger.error(f"Extraction task {task.task_id} failed: {e}")
            task.status = TaskStatus.FAILED
            task.error = str(e)
            task.completed_at = datetime.now().isoformat()

    def get_task_status(self, task_id: str) -> Optional[Dict[str, Any]]:
        """
        Get status of background extraction task.

        Args:
            task_id: Task identifier

        Returns:
            Task details dict or None if not found
        """
        with self._task_lock:
            task = self.tasks.get(task_id)

            if not task:
                return None

            return {
                "task_id": task.task_id,
                "source_path": task.source_path,
                "target_dir": task.target_dir,
                "status": task.status.value,
                "progress": task.progress,
                "current_file": task.current_file,
                "total_files": task.total_files,
                "error": task.error,
                "created_at": task.created_at,
                "started_at": task.started_at,
                "completed_at": task.completed_at
            }


# Singleton instance
_archive_service: Optional[ArchiveService] = None


def get_archive_service() -> ArchiveService:
    """
    Get or create ArchiveService singleton.

    Returns:
        ArchiveService instance
    """
    global _archive_service
    if _archive_service is None:
        _archive_service = ArchiveService()
    return _archive_service
