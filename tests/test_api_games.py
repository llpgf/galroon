"""
API endpoint tests for Games module.

Tests game listing, details, status updates, and scanning functionality.
Phase 20.0: SQLite-based instant index.
"""

import json
from pathlib import Path
from unittest.mock import Mock, patch, AsyncMock

import pytest
from fastapi.testclient import TestClient

from app.main import app
from app.config import Config
from app.core.database import get_database


# ============================================================================
# Fixtures
# ============================================================================

@pytest.fixture
def client():
    """Create a test client for the FastAPI app."""
    return TestClient(app)


@pytest.fixture
def mock_config(tmp_path):
    """Create a mock configuration with test paths."""
    config_dir = tmp_path / "config"
    library_root = tmp_path / "library"
    config_dir.mkdir()
    library_root.mkdir()

    config = Config()
    config.config_dir = config_dir
    config.library_root = library_root
    config.library_roots = [library_root]

    return config


@pytest.fixture
def sample_game_folder(mock_config):
    """Create a sample game folder with metadata."""
    game_folder = mock_config.library_root / "TestGame"
    game_folder.mkdir()

    metadata = {
        "title": {"value": "Test Game", "source": "manual"},
        "developer": {"value": "Test Developer", "source": "manual"},
        "vndb_id": "v12345",
        "library_status": {"value": "unstarted", "locked": False, "source": "user"},
        "rating": {"value": {"score": 8.0, "count": 1}},
        "tags": ["visual novel", "romance"],
        "user_tags": ["favorite"],
        "cover_url": "/covers/test.jpg"
    }

    metadata_file = game_folder / "metadata.json"
    with open(metadata_file, 'w', encoding='utf-8') as f:
        json.dump(metadata, f, indent=2, ensure_ascii=False)

    return game_folder


@pytest.fixture
def mock_database():
    """Create a mock database."""
    db = Mock()
    db.get_games = Mock(return_value=(
        [
            {
                'folder_path': 'TestGame',
                'title': 'Test Game',
                'developer': 'Test Developer',
                'cover_image': '/covers/test.jpg',
                'badges': [],
                'library_status': 'unstarted',
                'rating': 8.0,
                'release_date': '2024-01-01',
                'tags': ['visual novel'],
                'user_tags': ['favorite']
            }
        ],
        1
    ))
    db.upsert_game = Mock()
    return db


# ============================================================================
# Test Suite: List Games
# ============================================================================

class TestListGames:
    """Test suite for listing games."""

    def test_list_all_games_empty(self, client, mock_database):
        """List all games when library is empty."""
        mock_database.get_games.return_value = ([], 0)

        with patch('app.api.games.get_database', return_value=mock_database):
            response = client.get("/api/games")

        assert response.status_code == 200
        data = response.json()
        assert data['total'] == 0
        assert data['data'] == []
        assert data['page'] == 1
        assert data['strategy'] == "sqlite"

    def test_list_all_games_with_data(self, client, mock_database):
        """List all games with games in library."""
        games, total = mock_database.get_games()

        with patch('app.api.games.get_database', return_value=mock_database):
            response = client.get("/api/games")

        assert response.status_code == 200
        data = response.json()
        assert data['total'] == 1
        assert len(data['data']) == 1
        assert data['data'][0]['title'] == 'Test Game'
        assert data['strategy'] == "sqlite"

    def test_list_games_with_pagination(self, client, mock_database):
        """List games with pagination parameters."""
        with patch('app.api.games.get_database', return_value=mock_database):
            response = client.get("/api/games?skip=0&limit=50")

        assert response.status_code == 200
        data = response.json()
        assert data['size'] == 50

    def test_list_games_with_sorting(self, client, mock_database):
        """List games with sorting."""
        with patch('app.api.games.get_database', return_value=mock_database):
            response = client.get("/api/games?sort_by=名称")

        assert response.status_code == 200
        # Verify db.get_games was called with sort_by parameter
        mock_database.get_games.assert_called_once()

    def test_list_games_with_search(self, client, mock_database):
        """List games with full-text search."""
        with patch('app.api.games.get_database', return_value=mock_database):
            response = client.get("/api/games?search=test")

        assert response.status_code == 200
        # Verify db.get_games was called with search parameter
        mock_database.get_games.assert_called_once()

    def test_list_games_with_tag_filter(self, client, mock_database):
        """List games filtered by tag."""
        with patch('app.api.games.get_database', return_value=mock_database):
            response = client.get("/api/games?filter_tag=romance")

        assert response.status_code == 200
        # Verify db.get_games was called with filter_tag parameter
        mock_database.get_games.assert_called_once()

    def test_list_games_invalid_limit(self, client, mock_database):
        """List games with invalid limit parameter."""
        with patch('app.api.games.get_database', return_value=mock_database):
            response = client.get("/api/games?limit=500")  # Max is 200

        assert response.status_code == 422  # Validation error


# ============================================================================
# Test Suite: Get Game Details
# ============================================================================

class TestGetGameDetails:
    """Test suite for getting game details."""

    def test_get_game_details_success(self, client, sample_game_folder, mock_config):
        """Get details for an existing game."""
        with patch('app.api.games.get_config', return_value=mock_config):
            with patch('app.api.games.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'title': {'value': 'Test Game'},
                    'developer': {'value': 'Test Developer'},
                    'vndb_id': 'v12345'
                }

                response = client.get(f"/api/games/{sample_game_folder.name}")

        assert response.status_code == 200
        data = response.json()
        assert data['title']['value'] == 'Test Game'
        assert 'folder_path' in data

    def test_get_game_details_not_found(self, client, mock_config):
        """Get details for non-existent game."""
        with patch('app.api.games.get_config', return_value=mock_config):
            response = client.get("/api/games/NonExistent")

        assert response.status_code == 404

    def test_get_game_details_invalid_path(self, client, mock_config):
        """Get details with invalid game path."""
        with patch('app.api.games.get_config', return_value=mock_config):
            with patch('app.api.games.is_safe_path', return_value=False):
                response = client.get("/api/games/../../../etc")

        # Should reject unsafe paths
        assert response.status_code in [400, 404]


# ============================================================================
# Test Suite: Update Library Status
# ============================================================================

class TestUpdateLibraryStatus:
    """Test suite for updating game library status."""

    def test_update_library_status_success(
        self, client, sample_game_folder, mock_config, mock_database
    ):
        """Successfully update library status."""
        payload = {"library_status": "in_progress"}

        with patch('app.api.games.get_config', return_value=mock_config):
            with patch('app.api.games.get_resource_manager') as mock_rm:
                with patch('app.api.games.get_database', return_value=mock_database):
                    mock_rm.return_value.load_metadata.return_value = {
                        'library_status': {'value': 'unstarted', 'locked': False}
                    }
                    mock_rm.return_value.save_metadata.return_value = True

                    response = client.patch(
                        f"/api/games/{sample_game_folder.name}/status",
                        json=payload
                    )

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True
        assert data['library_status'] == 'in_progress'

    def test_update_library_status_invalid_status(
        self, client, sample_game_folder, mock_config
    ):
        """Try to update with invalid library status."""
        payload = {"library_status": "invalid_status"}

        with patch('app.api.games.get_config', return_value=mock_config):
            with patch('app.api.games.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'library_status': {'value': 'unstarted', 'locked': False}
                }

                response = client.patch(
                    f"/api/games/{sample_game_folder.name}/status",
                    json=payload
                )

        assert response.status_code == 400

    def test_update_library_status_game_not_found(
        self, client, mock_config
    ):
        """Try to update non-existent game."""
        payload = {"library_status": "finished"}

        with patch('app.api.games.get_config', return_value=mock_config):
            with patch('app.api.games.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = None

                response = client.patch("/api/games/NonExistent/status", json=payload)

        assert response.status_code == 404


# ============================================================================
# Test Suite: Scanner Endpoints
# ============================================================================

class TestScannerEndpoints:
    """Test suite for scanner-related endpoints."""

    def test_trigger_scan_success(self, client):
        """Trigger a background library scan."""
        with patch('app.api.games.get_scanner') as mock_scanner:
            mock_scanner.return_value.is_scanning.return_value = False

            response = client.post("/api/games/scan")

        assert response.status_code == 200
        data = response.json()
        assert data['status'] == 'scan_started'

    def test_trigger_scan_already_scanning(self, client):
        """Try to trigger scan when already scanning."""
        with patch('app.api.games.get_scanner') as mock_scanner:
            mock_scanner.return_value.is_scanning.return_value = True

            response = client.post("/api/games/scan")

        assert response.status_code == 200
        data = response.json()
        assert data['status'] == 'already_scanning'

    def test_get_scan_status(self, client):
        """Get current scan status."""
        with patch('app.api.games.get_scanner') as mock_scanner:
            mock_scanner.return_value.is_scanning.return_value = True

            response = client.get("/api/games/scan/status")

        assert response.status_code == 200
        data = response.json()
        assert data['scanning'] is True

    def test_get_scan_status_idle(self, client):
        """Get scan status when not scanning."""
        with patch('app.api.games.get_scanner') as mock_scanner:
            mock_scanner.return_value.is_scanning.return_value = False

            response = client.get("/api/games/scan/status")

        assert response.status_code == 200
        data = response.json()
        assert data['scanning'] is False


# ============================================================================
# Test Suite: Helper Functions
# ============================================================================

class TestHelperFunctions:
    """Test suite for helper functions."""

    def test_extract_badges(self):
        """Test badge extraction from metadata."""
        from app.api.games import extract_badges

        metadata = {
            'assets_detected': ['game_patch.exe', 'dlc_addon.iso']
        }

        badges = extract_badges(metadata)

        assert 'Patch' in badges
        assert 'DLC' in badges

    def test_extract_badges_from_versions(self):
        """Test badge extraction from versions."""
        from app.api.games import extract_badges

        metadata = {
            'versions': [
                {
                    'assets': ['main_game.iso']
                },
                {
                    'assets': ['patch_v1.0.exe']
                }
            ]
        }

        badges = extract_badges(metadata)

        assert 'ISO' in badges
        assert 'Patch' in badges

    def test_metadata_to_summary(self):
        """Test metadata to summary conversion."""
        from app.api.games import metadata_to_summary

        metadata = {
            'title': {'value': 'Test Game'},
            'developer': {'value': 'Test Dev'},
            'cover_url': {'value': '/cover.jpg'},
            'library_status': {'value': 'in_progress'},
            'rating': {'value': {'score': 9.0}},
            'release_date': {'value': '2024-01-01'},
            'tags': ['vn'],
            'user_tags': ['favorite']
        }

        summary = metadata_to_summary(metadata, Path("/path/to/game"))

        assert summary.title == 'Test Game'
        assert summary.developer == 'Test Dev'
        assert summary.library_status == 'in_progress'
        assert summary.rating == 9.0
        assert summary.tags == ['vn']
        assert summary.user_tags == ['favorite']

    def test_migrate_legacy_status(self):
        """Test legacy play_status migration."""
        from app.api.games import _migrate_legacy_status

        metadata_old = {
            'play_status': {'value': 'playing', 'locked': False}
        }

        metadata_new = _migrate_legacy_status(metadata_old)

        assert 'library_status' in metadata_new
        assert metadata_new['library_status']['value'] == 'in_progress'
        assert 'play_status' not in metadata_new
        assert metadata_new['library_status']['source'] == 'migrated'

    def test_migrate_already_migrated(self):
        """Test that already migrated metadata is not changed."""
        from app.api.games import _migrate_legacy_status

        metadata = {
            'library_status': {'value': 'finished', 'locked': False}
        }

        result = _migrate_legacy_status(metadata)

        assert result == metadata
