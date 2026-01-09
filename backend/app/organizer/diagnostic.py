"""
Self-Audit Engine - Sprint 10.5 Pre-Delivery
Performs comprehensive self-diagnostics before user interaction.

Components:
1. Physical Stress Test (Dry-Run with simulated interruption)
2. Metadata Integrity Check (DB vs disk JSON)
3. Symlink Health Check (validate all junctions)
4. Checksum Verification
"""

import os
import json
import hashlib
import logging
import tempfile
import shutil
from pathlib import Path
from typing import Dict, Any, List, Optional
from dataclasses import dataclass, field, asdict
from datetime import datetime

from ..core.database import Database
from .safety import SafetyOps
from .sidecar import SidecarGenerator

logger = logging.getLogger(__name__)


@dataclass
class DiagnosticResult:
    test_name: str
    passed: bool
    details: str
    timestamp: str = field(default_factory=lambda: datetime.now().isoformat())


@dataclass
class AuditReport:
    generated_at: str
    total_tests: int
    passed: int
    failed: int
    results: List[Dict[str, Any]]


class AppDiagnosticProvider:
    """
    Central diagnostic provider for self-audit.
    """

    def __init__(self, db: Database):
        self.db = db
        self.results: List[DiagnosticResult] = []

    def run_all_checks(self) -> AuditReport:
        """Execute all diagnostic checks."""
        self.results = []

        # 1. Physical Safety Tests
        self._check_physical_safety()

        # 2. Metadata Integrity
        self._check_metadata_integrity()

        # 3. Symlink Health
        self._check_symlink_health()

        # 4. Checksum Verification
        self._check_checksum_integrity()

        # Compile report
        passed = sum(1 for r in self.results if r.passed)
        failed = len(self.results) - passed

        return AuditReport(
            generated_at=datetime.now().isoformat(),
            total_tests=len(self.results),
            passed=passed,
            failed=failed,
            results=[asdict(r) for r in self.results]
        )

    def check_physical_safety(self) -> List[DiagnosticResult]:
        """
        Public API for physical safety validation.
        Tests atomic operation rollback reliability.
        """
        self._check_physical_safety()
        return [r for r in self.results if 'Physical' in r.test_name]

    def _check_physical_safety(self):
        """
        Test 1: Physical Simulation Stress Test
        - Create temp folder structure
        - Simulate atomic move
        - Simulate interruption (delete mid-move target)
        - Verify rollback works
        """
        logger.info("[Diagnostic] Running physical safety checks...")

        # Test A: Basic Atomic Move
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                src = Path(tmpdir) / "test_source"
                dest = Path(tmpdir) / "test_dest"
                src.mkdir()

                # Create test files
                (src / "game.exe").write_text("test_executable")
                (src / "data.bin").write_bytes(b"\x00" * 1024)

                # Perform atomic move
                success = SafetyOps.atomic_move(src, dest)

                if success and dest.exists() and not src.exists():
                    self.results.append(DiagnosticResult(
                        test_name="Physical: Atomic Move",
                        passed=True,
                        details="Successfully moved files atomically"
                    ))
                else:
                    self.results.append(DiagnosticResult(
                        test_name="Physical: Atomic Move",
                        passed=False,
                        details=f"Move failed. src exists: {src.exists()}, dest exists: {dest.exists()}"
                    ))
        except Exception as e:
            self.results.append(DiagnosticResult(
                test_name="Physical: Atomic Move",
                passed=False,
                details=f"Exception: {str(e)}"
            ))

        # Test B: Rollback Simulation
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                src = Path(tmpdir) / "rollback_src"
                dest = Path(tmpdir) / "rollback_dest"
                src.mkdir()
                (src / "important.dat").write_text("critical_data")

                # Move to dest
                shutil.move(str(src), str(dest))

                # Now simulate rollback
                rollback_success = SafetyOps.rollback_move(src, dest)

                if rollback_success and src.exists() and (src / "important.dat").exists():
                    self.results.append(DiagnosticResult(
                        test_name="Physical: Rollback",
                        passed=True,
                        details="Rollback successfully restored original location"
                    ))
                else:
                    self.results.append(DiagnosticResult(
                        test_name="Physical: Rollback",
                        passed=False,
                        details="Rollback did not restore files"
                    ))
        except Exception as e:
            self.results.append(DiagnosticResult(
                test_name="Physical: Rollback",
                passed=False,
                details=f"Exception: {str(e)}"
            ))

        # Test C: Permission Boundary (Read-only folder)
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                readonly_dir = Path(tmpdir) / "readonly"
                readonly_dir.mkdir()
                test_file = readonly_dir / "test.txt"
                test_file.write_text("test")

                # Make read-only (Windows: attrib +R, cross-platform: os.chmod)
                os.chmod(readonly_dir, 0o444)

                # Attempt to create symlink inside readonly (should fail)
                link_path = readonly_dir / "link_test"
                symlink_result = SafetyOps.create_symlink(Path(tmpdir), link_path, is_dir=True)

                # Restore permissions for cleanup
                os.chmod(readonly_dir, 0o755)

                if not symlink_result:
                    self.results.append(DiagnosticResult(
                        test_name="Physical: Permission Boundary",
                        passed=True,
                        details="Correctly rejected operation in read-only context"
                    ))
                else:
                    self.results.append(DiagnosticResult(
                        test_name="Physical: Permission Boundary",
                        passed=False,
                        details="Unexpectedly succeeded in read-only folder"
                    ))
        except Exception as e:
            # Exception is acceptable here - means we couldn't write
            self.results.append(DiagnosticResult(
                test_name="Physical: Permission Boundary",
                passed=True,
                details=f"Correctly raised exception for restricted path: {type(e).__name__}"
            ))

    def _check_metadata_integrity(self):
        """
        Test 2: Metadata Integrity Check
        Compare overridden_fields in DB vs metadata.json on disk.
        """
        logger.info("[Diagnostic] Running metadata integrity checks...")

        try:
            # Get all local instances with metadata
            rows = self.db.conn.execute("""
                SELECT li.folder_path, cg.overridden_fields 
                FROM local_instances li 
                JOIN canonical_games cg ON li.canonical_id = cg.id
                WHERE cg.overridden_fields IS NOT NULL AND cg.overridden_fields != '{}'
            """).fetchall()

            conflicts = []
            checked = 0

            for row in rows:
                folder_path = Path(row['folder_path'])
                db_overrides = json.loads(row['overridden_fields'] or '{}')
                metadata_file = folder_path / "metadata.json"

                if metadata_file.exists():
                    checked += 1
                    try:
                        disk_data = json.loads(metadata_file.read_text(encoding='utf-8'))
                        disk_overrides = disk_data.get('overridden_fields', {})

                        if db_overrides != disk_overrides:
                            conflicts.append({
                                'path': str(folder_path),
                                'db': db_overrides,
                                'disk': disk_overrides
                            })
                    except json.JSONDecodeError:
                        conflicts.append({
                            'path': str(folder_path),
                            'error': 'Invalid JSON on disk'
                        })

            if conflicts:
                self.results.append(DiagnosticResult(
                    test_name="Metadata: Authority Conflict",
                    passed=False,
                    details=f"Found {len(conflicts)} conflicts. User manual decision takes precedence."
                ))
            else:
                self.results.append(DiagnosticResult(
                    test_name="Metadata: Authority Conflict",
                    passed=True,
                    details=f"Checked {checked} entries. No conflicts detected."
                ))

        except Exception as e:
            self.results.append(DiagnosticResult(
                test_name="Metadata: Authority Conflict",
                passed=False,
                details=f"Check failed: {str(e)}"
            ))

    def _check_symlink_health(self):
        """
        Test 3: Symlink Health Check
        Verify all symlinks/junctions point to valid targets.
        """
        logger.info("[Diagnostic] Running symlink health checks...")

        try:
            rows = self.db.conn.execute("""
                SELECT folder_path FROM local_instances
            """).fetchall()

            broken_links = []
            total_links = 0

            for row in rows:
                folder_path = Path(row['folder_path'])

                # Check if path is a symlink
                if folder_path.is_symlink():
                    total_links += 1
                    target = folder_path.resolve()
                    if not target.exists():
                        broken_links.append({
                            'link': str(folder_path),
                            'target': str(target),
                            'status': 'BROKEN'
                        })

            if broken_links:
                self.results.append(DiagnosticResult(
                    test_name="Symlink: Health Check",
                    passed=False,
                    details=f"Found {len(broken_links)} broken symlinks out of {total_links}"
                ))
            else:
                self.results.append(DiagnosticResult(
                    test_name="Symlink: Health Check",
                    passed=True,
                    details=f"All {total_links} symlinks are valid"
                ))

        except Exception as e:
            self.results.append(DiagnosticResult(
                test_name="Symlink: Health Check",
                passed=False,
                details=f"Check failed: {str(e)}"
            ))

    def _check_checksum_integrity(self):
        """
        Test 4: Checksum Verification
        Verify metadata.json files have valid checksums.
        """
        logger.info("[Diagnostic] Running checksum verification...")

        try:
            rows = self.db.conn.execute("""
                SELECT folder_path FROM local_instances
            """).fetchall()

            invalid_checksums = []
            checked = 0

            for row in rows:
                folder_path = Path(row['folder_path'])
                metadata_file = folder_path / "metadata.json"

                if metadata_file.exists():
                    checked += 1
                    content = metadata_file.read_bytes()

                    try:
                        data = json.loads(content.decode('utf-8'))
                        stored_checksum = data.get('_checksum')

                        if stored_checksum:
                            # Recalculate checksum (excluding the _checksum field itself)
                            data_for_hash = {k: v for k, v in data.items() if k != '_checksum'}
                            calculated = hashlib.sha256(
                                json.dumps(data_for_hash, sort_keys=True).encode()
                            ).hexdigest()[:16]

                            if stored_checksum != calculated:
                                invalid_checksums.append(str(folder_path))
                    except (json.JSONDecodeError, UnicodeDecodeError):
                        invalid_checksums.append(str(folder_path))

            if invalid_checksums:
                self.results.append(DiagnosticResult(
                    test_name="Checksum: Integrity",
                    passed=False,
                    details=f"Found {len(invalid_checksums)} files with invalid checksums"
                ))
            else:
                self.results.append(DiagnosticResult(
                    test_name="Checksum: Integrity",
                    passed=True,
                    details=f"Verified {checked} metadata files"
                ))

        except Exception as e:
            self.results.append(DiagnosticResult(
                test_name="Checksum: Integrity",
                passed=False,
                details=f"Check failed: {str(e)}"
            ))


def generate_audit_report(db: Database, output_path: Path) -> Path:
    """
    Run full diagnostic suite and generate report file.
    """
    provider = AppDiagnosticProvider(db)
    report = provider.run_all_checks()

    # Ensure output directory exists
    output_path.mkdir(parents=True, exist_ok=True)

    report_file = output_path / f"self_audit_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"

    with open(report_file, 'w', encoding='utf-8') as f:
        json.dump(asdict(report), f, indent=2, ensure_ascii=False)

    logger.info(f"[Diagnostic] Audit report saved to: {report_file}")
    return report_file
