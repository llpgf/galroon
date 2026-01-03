"""
Background Scheduler Service for Galgame Library Manager.

Phase 24.5: System Governance - Scheduled Tasks

Features:
- APScheduler integration for reliable job scheduling
- Configurable intervals (persisted to config.json)
- Startup hook initialization
- Jobs: Library scan, Backup creation, Cache cleanup
"""

import logging
import os
import shutil
import time
from datetime import datetime, timedelta
from typing import Optional, Dict, Any
from pathlib import Path
from apscheduler.schedulers.background import BackgroundScheduler
from apscheduler.triggers.interval import IntervalTrigger
from apscheduler.triggers.cron import CronTrigger
from apscheduler.events import EVENT_JOB_EXECUTED, EVENT_JOB_ERROR

from ..config import get_config
from .scanner import get_scanner
from .backup import get_backup_manager

logger = logging.getLogger(__name__)


class TaskScheduler:
    """
    Background task scheduler with APScheduler.

    Thread-safe: Can be run in background while UI reads from DB.

    Phase 24.5: Enterprise-grade scheduling with configurable intervals.
    """

    def __init__(self):
        """Initialize scheduler."""
        self.config = get_config()
        self.scanner = get_scanner()
        self.scheduler = BackgroundScheduler()

        # Listen to job events for logging
        self.scheduler.add_listener(
            self._job_executed_listener,
            EVENT_JOB_EXECUTED | EVENT_JOB_ERROR
        )

        # Job store (in-memory by default)
        self._jobs: Dict[str, Any] = {}

    def _job_executed_listener(self, event):
        """Log job execution events."""
        if event.exception:
            logger.error(f"Job {event.job_id} failed: {event.exception}")
        else:
            logger.info(f"Job {event.job_id} executed successfully")

    def start(self):
        """
        Start the scheduler and add all jobs.

        Should be called during application startup.
        """
        if self.scheduler.running:
            logger.warning("Scheduler is already running")
            return

        # Add scheduled jobs
        self._schedule_library_scan()
        self._schedule_backup()
        self._schedule_cache_cleanup()

        # Start scheduler
        self.scheduler.start()
        logger.info("Task scheduler started")

    def shutdown(self, wait=True):
        """
        Shutdown the scheduler.

        Args:
            wait: Wait for all jobs to complete (default: True)
        """
        if not self.scheduler.running:
            logger.warning("Scheduler is not running")
            return

        self.scheduler.shutdown(wait=wait)
        logger.info("Task scheduler shut down")

    def _schedule_library_scan(self):
        """
        Schedule periodic library scan.

        Interval: Configurable via config.json (scan_interval_min)
        Default: 0 (manual mode - no automatic scanning)
        """
        interval_min = self.config.scan_interval_min

        if interval_min <= 0:
            logger.info("Library scan is set to manual mode (no automatic scanning)")
            return

        # Add interval job
        job = self.scheduler.add_job(
            self._run_library_scan,
            trigger=IntervalTrigger(minutes=interval_min),
            id='library_scan',
            name='Library Scan',
            replace_existing=True
        )

        self._jobs['library_scan'] = job
        logger.info(f"Scheduled library scan every {interval_min} minutes")

    def _schedule_backup(self):
        """
        Schedule daily backup creation.

        Phase 24.5: Time loaded from config (settings.json)
        Default: 03:00 AM
        """
        # Load backup time from config
        backup_hour = self.config.backup_hour
        backup_minute = self.config.backup_minute

        # Add cron job
        job = self.scheduler.add_job(
            self._run_backup,
            trigger=CronTrigger(hour=backup_hour, minute=backup_minute),
            id='backup',
            name='Daily Backup',
            replace_existing=True
        )

        self._jobs['backup'] = job
        logger.info(f"Scheduled daily backup at {backup_hour:02d}:{backup_minute:02d}")

    def _schedule_cache_cleanup(self):
        """
        Schedule weekly cache cleanup.

        Day: Sunday at 04:00 AM
        """
        # Add cron job
        job = self.scheduler.add_job(
            self._run_cache_cleanup,
            trigger=CronTrigger(day_of_week='sun', hour=4, minute=0),
            id='cache_cleanup',
            name='Weekly Cache Cleanup',
            replace_existing=True
        )

        self._jobs['cache_cleanup'] = job
        logger.info("Scheduled weekly cache cleanup (Sunday 04:00 AM)")

    def _run_library_scan(self):
        """Execute library scan job."""
        logger.info("Running scheduled library scan...")
        try:
            result = self.scanner.scan_library(background=False)
            logger.info(f"Scheduled scan complete: {result}")
        except Exception as e:
            logger.error(f"Scheduled scan failed: {e}", exc_info=True)

    def _run_backup(self):
        """Execute backup job."""
        logger.info("Running scheduled backup...")
        try:
            # Phase 24.5: Use backup service
            backup_manager = get_backup_manager()
            backup_meta = backup_manager.create_backup()
            logger.info(f"Scheduled backup complete: {backup_meta.filename}")
        except Exception as e:
            logger.error(f"Scheduled backup failed: {e}", exc_info=True)

    def _run_cache_cleanup(self):
        """
        Execute cache cleanup job.

        Phase 24.5: Cleans up:
        - Old journal entries (older than 30 days)
        - Temporary files in config directory
        - Empty directories in trash
        """
        logger.info("Running scheduled cache cleanup...")
        try:
            cleanup_stats = {
                'journal_entries_removed': 0,
                'temp_files_removed': 0,
                'empty_dirs_removed': 0,
                'total_size_freed_mb': 0
            }

            # 1. Clean up old journal entries (older than 30 days)
            journal_dir = self.config.journal_dir
            if journal_dir.exists():
                cutoff_time = time.time() - (30 * 24 * 60 * 60)  # 30 days ago
                for journal_file in journal_dir.glob("journal_*.db"):
                    try:
                        if journal_file.stat().st_mtime < cutoff_time:
                            size_bytes = journal_file.stat().st_size
                            journal_file.unlink()
                            cleanup_stats['journal_entries_removed'] += 1
                            cleanup_stats['total_size_freed_mb'] += size_bytes / (1024 * 1024)
                            logger.info(f"Removed old journal file: {journal_file.name}")
                    except Exception as e:
                        logger.warning(f"Failed to remove journal file {journal_file}: {e}")

            # 2. Clean up temporary files
            config_dir = self.config.config_dir
            for temp_pattern in ['*.tmp', '*.temp', '*.bak']:
                for temp_file in config_dir.glob(temp_pattern):
                    try:
                        size_bytes = temp_file.stat().st_size
                        temp_file.unlink()
                        cleanup_stats['temp_files_removed'] += 1
                        cleanup_stats['total_size_freed_mb'] += size_bytes / (1024 * 1024)
                        logger.info(f"Removed temp file: {temp_file.name}")
                    except Exception as e:
                        logger.warning(f"Failed to remove temp file {temp_file}: {e}")

            # 3. Clean up empty directories in trash
            trash_dir = self.config.trash_dir
            if trash_dir.exists():
                for root, dirs, files in os.walk(trash_dir, topdown=False):
                    for dir_name in dirs:
                        dir_path = Path(root) / dir_name
                        try:
                            # Check if directory is empty
                            if not any(dir_path.iterdir()):
                                dir_path.rmdir()
                                cleanup_stats['empty_dirs_removed'] += 1
                                logger.info(f"Removed empty directory: {dir_path}")
                        except Exception as e:
                            logger.warning(f"Failed to remove empty dir {dir_path}: {e}")

            logger.info(f"Scheduled cache cleanup complete: {cleanup_stats}")
        except Exception as e:
            logger.error(f"Scheduled cache cleanup failed: {e}", exc_info=True)

    def update_scan_interval(self, interval_min: int):
        """
        Update library scan interval.

        Args:
            interval_min: New interval in minutes (0 = manual mode)
        """
        # Remove existing job
        if 'library_scan' in self._jobs:
            self.scheduler.remove_job('library_scan')
            del self._jobs['library_scan']

        # Reschedule with new interval
        self.config.scan_interval_min = interval_min
        self._schedule_library_scan()

        logger.info(f"Library scan interval updated to {interval_min} minutes")

    def update_backup_time(self, hour: int, minute: int):
        """
        Update backup schedule time.

        Phase 24.5: Reschedules backup job with new time

        Args:
            hour: Backup hour (0-23)
            minute: Backup minute (0-59)
        """
        # Remove existing job
        if 'backup' in self._jobs:
            self.scheduler.remove_job('backup')
            del self._jobs['backup']

        # Update config and reschedule
        self.config.update_backup_settings(hour=hour, minute=minute)
        self._schedule_backup()

        logger.info(f"Backup time updated to {hour:02d}:{minute:02d}")

    def trigger_scan_now(self):
        """
        Trigger library scan immediately (one-time).

        Useful for manual scan trigger from UI.
        """
        logger.info("Triggering manual scan...")
        self.scanner.scan_library(background=True)

    def get_next_run_time(self, job_id: str) -> Optional[datetime]:
        """
        Get next scheduled run time for a job.

        Args:
            job_id: Job identifier (e.g., 'library_scan', 'backup', 'cache_cleanup')

        Returns:
            Next run time or None if job not found
        """
        job = self.scheduler.get_job(job_id)
        if job:
            return job.next_run_time
        return None

    def get_job_info(self) -> Dict[str, Dict[str, Any]]:
        """
        Get information about all scheduled jobs.

        Returns:
            Dict with job details
        """
        jobs = {}

        for job in self.scheduler.get_jobs():
            jobs[job.id] = {
                'name': job.name,
                'next_run_time': job.next_run_time.isoformat() if job.next_run_time else None,
                'trigger': str(job.trigger),
            }

        return jobs


# Global scheduler instance
_scheduler: TaskScheduler = None


def get_scheduler() -> TaskScheduler:
    """
    Get or create global scheduler instance.

    Returns:
        TaskScheduler singleton
    """
    global _scheduler
    if _scheduler is None:
        _scheduler = TaskScheduler()
    return _scheduler


def startup_hook():
    """
    Startup hook for scheduler initialization.

    Should be called during application startup.

    Example:
        from app.services.scheduler import startup_hook, shutdown_hook

        @app.on_event("startup")
        async def startup():
            startup_hook()

        @app.on_event("shutdown")
        async def shutdown():
            shutdown_hook()
    """
    scheduler = get_scheduler()
    scheduler.start()


def shutdown_hook():
    """
    Shutdown hook for scheduler cleanup.

    Should be called during application shutdown.
    """
    global _scheduler

    if _scheduler is not None:
        _scheduler.shutdown(wait=True)
        _scheduler = None
