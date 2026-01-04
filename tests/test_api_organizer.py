"""
API endpoint tests for Organizer module.

Tests organization proposal generation, execution, rollback, and analysis.
Phase 9.5: The Curator Workbench.
"""

from pathlib import Path
from unittest.mock import Mock, patch, MagicMock
from typing import Dict, Any

import pytest
from fastapi.testclient import TestClient

from app.main import app


# ============================================================================
# Fixtures
# ============================================================================

@pytest.fixture
def client():
    """Create a test client for FastAPI app."""
    return TestClient(app)


@pytest.fixture
def tmp_test_dir(tmp_path):
    """Create temporary test directories."""
    source = tmp_path / "source"
    target = tmp_path / "target"
    source.mkdir()
    target.mkdir()

    # Create test files
    (source / "game.iso").write_text("iso content")
    (source / "patch.exe").write_text("patch content")

    return source, target


@pytest.fixture
def sample_vndb_metadata():
    """Sample VNDB metadata."""
    return {
        'title': 'Test Visual Novel',
        'original': 'テストビジュアルノベル',
        'developer': 'Test Studio',
        'release_date': '2024-01-01',
        'vndb_id': 'v12345',
        'length': '10-30 hours',
        'description': 'Test description'
    }


# ============================================================================
# Test Suite: Generate Proposal
# ============================================================================

class TestGenerateProposal:
    """Test suite for generating organization proposals."""

    def test_generate_proposal_success(
        self, client, tmp_test_dir, sample_vndb_metadata
    ):
        """Generate organization proposal successfully."""
        source, target = tmp_test_dir

        request = {
            'source_path': str(source),
            'target_root': str(target),
            'vndb_metadata': sample_vndb_metadata
        }

        with patch('app.api.organizer.AssetDetector') as MockDetector:
            with patch('app.api.organizer.generate_proposal') as mock_generate:
                # Mock proposal
                mock_proposal = Mock()
                mock_proposal.proposal_id = "test-proposal-123"
                mock_proposal.source_path = source
                mock_proposal.target_structure = {}
                mock_proposal.vndb_metadata = sample_vndb_metadata
                mock_proposal.moves = []
                mock_proposal.categorized_moves = {}
                mock_proposal.archive_groups = []
                mock_proposal.unresolved_files = []
                mock_proposal.created_at = "2024-01-01T00:00:00"
                mock_proposal.get_summary.return_value = {
                    'total_files': 2,
                    'resolved': 2,
                    'unresolved': 0
                }

                mock_generate.return_value = mock_proposal

                response = client.post("/api/organizer/generate", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['proposal_id'] == 'test-proposal-123'
        assert data['source_path'] == str(source)

    def test_generate_proposal_source_not_found(self, client, tmp_path):
        """Try to generate proposal with non-existent source."""
        request = {
            'source_path': str(tmp_path / "nonexistent"),
            'target_root': str(tmp_path),
            'vndb_metadata': {}
        }

        response = client.post("/api/organizer/generate", json=request)

        assert response.status_code == 404

    def test_generate_proposal_target_not_found(self, client, tmp_path):
        """Try to generate proposal with non-existent target."""
        source = tmp_path / "source"
        source.mkdir()

        request = {
            'source_path': str(source),
            'target_root': str(tmp_path / "nonexistent"),
            'vndb_metadata': {}
        }

        response = client.post("/api/organizer/generate", json=request)

        assert response.status_code == 404


# ============================================================================
# Test Suite: Execute Proposal
# ============================================================================

class TestExecuteProposal:
    """Test suite for executing organization proposals."""

    def test_execute_proposal_success(self, client):
        """Execute organization proposal successfully."""
        proposal = {
            'proposal_id': 'test-proposal-123',
            'source_path': '/path/to/source',
            'target_structure': {},
            'vndb_metadata': {},
            'moves': [
                {
                    'source': '/path/to/source/file.iso',
                    'target': '/path/to/target/file.iso',
                    'status': 'safe',
                    'category': 'Game',
                    'reason': 'Main game file',
                    'size': 1024
                }
            ],
            'categorized_moves': {},
            'archive_groups': [],
            'unresolved_files': [],
            'created_at': '2024-01-01T00:00:00'
        }

        request = {
            'proposal': proposal,
            'skip_unresolved': True,
            'cleanup_empty_dirs': True
        }

        with patch('app.api.organizer.execute_plan') as mock_execute:
            mock_result = Mock()
            mock_result.success = True
            mock_result.proposal_id = 'test-proposal-123'
            mock_result.moved_count = 1
            mock_result.skipped_count = 0
            mock_result.failed_count = 0
            mock_result.errors = []
            mock_result.undo_log_path = Path('/path/to/undo.json')
            mock_result.created_at = '2024-01-01T00:00:00'

            mock_execute.return_value = mock_result

            response = client.post("/api/organizer/execute", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True
        assert data['moved_count'] == 1

    def test_execute_proposal_with_errors(self, client):
        """Execute proposal with some errors."""
        proposal = {
            'proposal_id': 'test-proposal-456',
            'source_path': '/path/to/source',
            'target_structure': {},
            'vndb_metadata': {},
            'moves': [],
            'categorized_moves': {},
            'archive_groups': [],
            'unresolved_files': [],
            'created_at': '2024-01-01T00:00:00'
        }

        request = {
            'proposal': proposal,
            'skip_unresolved': False,
            'cleanup_empty_dirs': False
        }

        with patch('app.api.organizer.execute_plan') as mock_execute:
            mock_result = Mock()
            mock_result.success = True
            mock_result.proposal_id = 'test-proposal-456'
            mock_result.moved_count = 1
            mock_result.skipped_count = 1
            mock_result.failed_count = 0
            mock_result.errors = ['Warning: Some files were skipped']
            mock_result.undo_log_path = Path('/path/to/undo.json')
            mock_result.created_at = '2024-01-01T00:00:00'

            mock_execute.return_value = mock_result

            response = client.post("/api/organizer/execute", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['errors'] == ['Warning: Some files were skipped']


# ============================================================================
# Test Suite: Rollback
# ============================================================================

class TestRollback:
    """Test suite for rolling back organization changes."""

    def test_rollback_success(self, client, tmp_path):
        """Rollback executed proposal successfully."""
        undo_log = tmp_path / "undo_log.json"
        undo_log.write_text("{}")

        request = {'undo_log_path': str(undo_log)}

        with patch('app.api.organizer.rollback') as mock_rollback:
            mock_rollback.return_value = True

            response = client.post("/api/organizer/rollback", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True
        assert 'completed successfully' in data['message'].lower()

    def test_rollback_with_errors(self, client, tmp_path):
        """Rollback with some errors."""
        undo_log = tmp_path / "undo_log.json"
        undo_log.write_text("{}")

        request = {'undo_log_path': str(undo_log)}

        with patch('app.api.organizer.rollback') as mock_rollback:
            mock_rollback.return_value = False

            response = client.post("/api/organizer/rollback", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is False
        assert 'errors' in data['message'].lower()

    def test_rollback_log_not_found(self, client, tmp_path):
        """Try to rollback with non-existent undo log."""
        request = {
            'undo_log_path': str(tmp_path / "nonexistent.json")
        }

        response = client.post("/api/organizer/rollback", json=request)

        assert response.status_code == 404


# ============================================================================
# Test Suite: Analyze Directory
# ============================================================================

class TestAnalyzeDirectory:
    """Test suite for directory analysis."""

    def test_analyze_directory_success(self, client, tmp_path):
        """Analyze directory for assets successfully."""
        test_dir = tmp_path / "test_game"
        test_dir.mkdir()

        (test_dir / "game.exe").write_text("game")
        (test_dir / "patch.exe").write_text("patch")
        (test_dir / "manual.pdf").write_text("manual")

        request = {'path': str(test_dir)}

        with patch('app.api.organizer.AssetDetector') as MockDetector:
            mock_detector = Mock()
            mock_result = Mock()
            mock_result.assets = ['game.exe', 'patch.exe', 'manual.pdf']
            mock_result.version_label = 'v1.0'
            mock_result.file_count = 3
            mock_result.matched_patterns = {
                'exe': 2,
                'pdf': 1
            }

            mock_detector.return_value.detect_directory.return_value = mock_result

            with patch('app.api.organizer.AssetDetector', return_value=mock_detector):
                response = client.post("/api/organizer/analyze", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['path'] == str(test_dir)
        assert len(data['detected_assets']) == 3
        assert data['version_label'] == 'v1.0'
        assert data['file_count'] == 3

    def test_analyze_directory_not_found(self, client, tmp_path):
        """Try to analyze non-existent directory."""
        request = {'path': str(tmp_path / "nonexistent")}

        response = client.post("/api/organizer/analyze", json=request)

        assert response.status_code == 404


# ============================================================================
# Test Suite: Get Standards
# ============================================================================

class TestGetStandards:
    """Test suite for getting organization standards."""

    def test_get_standards(self, client):
        """Get organization standards information."""
        response = client.get("/api/organizer/standards")

        assert response.status_code == 200
        data = response.json()

        assert 'standard_format' in data
        assert 'subdirectories' in data
        assert 'features' in data

        # Verify standard format
        assert 'Library_Root' in data['standard_format']
        assert 'Developer' in data['standard_format']
        assert 'Year' in data['standard_format']
        assert 'Title' in data['standard_format']
        assert 'VNDB_ID' in data['standard_format']

        # Verify subdirectories
        assert 'Game' in data['subdirectories']
        assert 'Repository' in data['subdirectories']
        assert 'Patch_Work' in data['subdirectories']
        assert 'Extras' in data['subdirectories']
        assert 'Metadata' in data['subdirectories']

        # Verify features
        assert len(data['features']) > 0


# ============================================================================
# Test Suite: Error Handling
# ============================================================================

class TestErrorHandling:
    """Test suite for error handling."""

    def test_invalid_request_format(self, client):
        """Send invalid request format."""
        response = client.post(
            "/api/organizer/generate",
            json={"invalid": "data"}
        )

        assert response.status_code == 422

    def test_missing_required_field(self, client):
        """Send request missing required field."""
        response = client.post(
            "/api/organizer/generate",
            json={"source_path": "/some/path"}
        )

        assert response.status_code == 422


# ============================================================================
# Test Suite: Integration Tests
# ============================================================================

class TestOrganizerIntegration:
    """Integration tests for organizer workflow."""

    def test_full_workflow(self, client, tmp_path):
        """Test complete organization workflow."""
        # Step 1: Generate proposal
        source = tmp_path / "source"
        target = tmp_path / "target"
        source.mkdir()
        target.mkdir()

        (source / "game.iso").write_text("game")

        generate_request = {
            'source_path': str(source),
            'target_root': str(target),
            'vndb_metadata': {
                'title': 'Test Game',
                'developer': 'Test Dev',
                'vndb_id': 'v12345'
            }
        }

        with patch('app.api.organizer.AssetDetector') as MockDetector:
            with patch('app.api.organizer.generate_proposal') as mock_generate:
                mock_proposal = Mock()
                mock_proposal.proposal_id = "test-123"
                mock_proposal.source_path = source
                mock_proposal.target_structure = {}
                mock_proposal.vndb_metadata = {}
                mock_proposal.moves = []
                mock_proposal.categorized_moves = {}
                mock_proposal.archive_groups = []
                mock_proposal.unresolved_files = []
                mock_proposal.created_at = "2024-01-01"
                mock_proposal.get_summary.return_value = {}

                mock_generate.return_value = mock_proposal

                response = client.post("/api/organizer/generate", json=generate_request)

        assert response.status_code == 200
        proposal_data = response.json()

        # Step 2: Execute proposal
        execute_request = {
            'proposal': proposal_data,
            'skip_unresolved': True,
            'cleanup_empty_dirs': True
        }

        with patch('app.api.organizer.execute_plan') as mock_execute:
            mock_result = Mock()
            mock_result.success = True
            mock_result.proposal_id = 'test-123'
            mock_result.moved_count = 1
            mock_result.skipped_count = 0
            mock_result.failed_count = 0
            mock_result.errors = []
            mock_result.undo_log_path = Path('/undo.json')
            mock_result.created_at = '2024-01-01'

            mock_execute.return_value = mock_result

            response = client.post("/api/organizer/execute", json=execute_request)

        assert response.status_code == 200
        assert response.json()['success'] is True
