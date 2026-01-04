"""
API endpoint tests for Curator module.

Tests game identification, field locking, extras, and version management.
Phase 10: The Curator Backend.
Phase 18.5: Custom User Tags.
Phase 19.6: Version Manager APIs.
Phase 24.0: Smart Merge & Image Management.
"""

import json
from pathlib import Path
from unittest.mock import Mock, patch, AsyncMock

import pytest
from fastapi.testclient import TestClient

from app.main import app
from app.config import Config


# ============================================================================
# Fixtures
# ============================================================================

@pytest.fixture
def client():
    """Create a test client for FastAPI app."""
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
        'title': {'value': 'Test Game', 'source': 'manual'},
        'developer': {'value': 'Test Developer', 'source': 'manual'},
        'vndb_id': 'v12345',
        'library_status': {'value': 'unstarted', 'locked': False, 'source': 'user'},
        'tags': ['visual novel'],
        'user_tags': ['favorite'],
        'cover_url': {'value': '/covers/test.jpg', 'source': 'manual'},
        'locked_fields': []
    }

    metadata_file = game_folder / "galgame_metadata.json"
    with open(metadata_file, 'w', encoding='utf-8') as f:
        json.dump(metadata, f, indent=2, ensure_ascii=False)

    return game_folder


# ============================================================================
# Test Suite: Identify Game
# ============================================================================

class TestIdentifyGame:
    """Test suite for manual game identification."""

    def test_identify_game_success(
        self, client, sample_game_folder, mock_config
    ):
        """Successfully identify a game with VNDB ID."""
        request = {
            'folder_path': str(sample_game_folder),
            'vndb_id': 'v12345',
            'fetch_metadata': True
        }

        mock_curator = Mock()
        mock_result = Mock()
        mock_result.success = True
        mock_result.folder_path = sample_game_folder
        mock_result.vndb_id = 'v12345'
        mock_result.metadata = Mock()
        mock_result.metadata.model_dump.return_value = {
            'title': {'value': 'Identified Game'},
            'vndb_id': 'v12345'
        }
        mock_result.merged = False
        mock_result.message = 'Game identified successfully'

        mock_curator.identify_game.return_value = mock_result

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_curator', return_value=mock_curator):
                response = client.post("/api/curator/identify", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True
        assert data['vndb_id'] == 'v12345'

    def test_identify_game_not_found(
        self, client, mock_config
    ):
        """Try to identify non-existent game."""
        request = {
            'folder_path': str(mock_config.library_root / "NonExistent"),
            'vndb_id': 'v12345',
            'fetch_metadata': False
        }

        response = client.post("/api/curator/identify", json=request)

        assert response.status_code == 500


# ============================================================================
# Test Suite: Lock/Unlock Fields
# ============================================================================

class TestLockFields:
    """Test suite for locking metadata fields."""

    def test_lock_fields_success(
        self, client, sample_game_folder, mock_config
    ):
        """Successfully lock metadata fields."""
        request = {
            'folder_path': str(sample_game_folder),
            'field_names': ['title', 'developer']
        }

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'title': {'value': 'Test Game'},
                    'developer': {'value': 'Test Dev'},
                    'locked_fields': []
                }

                mock_metadata = Mock()
                mock_metadata.get_locked_fields.return_value = ['title', 'developer']
                mock_rm.return_value.save_metadata.return_value = True

                with patch('app.api.curator.UnifiedMetadata') as MockMetadata:
                    MockMetadata.return_value = mock_metadata
                    mock_metadata.lock_fields.return_value = 2

                    response = client.post("/api/curator/lock_fields", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True
        assert data['locked_count'] == 2

    def test_unlock_fields_success(
        self, client, sample_game_folder, mock_config
    ):
        """Successfully unlock metadata fields."""
        request = {
            'folder_path': str(sample_game_folder),
            'field_names': ['title']
        }

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'title': {'value': 'Test Game', 'locked': True},
                    'locked_fields': ['title']
                }

                mock_metadata = Mock()
                mock_metadata.get_locked_fields.return_value = []
                mock_rm.return_value.save_metadata.return_value = True

                with patch('app.api.curator.UnifiedMetadata') as MockMetadata:
                    MockMetadata.return_value = mock_metadata
                    mock_metadata.unlock_fields.return_value = 1

                    response = client.post("/api/curator/unlock_fields", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True


# ============================================================================
# Test Suite: Update Field
# ============================================================================

class TestUpdateField:
    """Test suite for updating single metadata field."""

    def test_update_field_success(
        self, client, sample_game_folder, mock_config
    ):
        """Successfully update a single field."""
        request = {
            'folder_path': str(sample_game_folder),
            'field_name': 'title',
            'value': 'Updated Title',
            'lock_after_update': False
        }

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'title': {'value': 'Test Game'},
                    'locked_fields': []
                }

                mock_metadata = Mock()
                mock_rm.return_value.save_metadata.return_value = True

                with patch('app.api.curator.UnifiedMetadata', return_value=mock_metadata):
                    response = client.post("/api/curator/update_field", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True
        assert data['field_name'] == 'title'

    def test_update_field_invalid(self, client, sample_game_folder, mock_config):
        """Try to update invalid field."""
        request = {
            'folder_path': str(sample_game_folder),
            'field_name': 'invalid_field',
            'value': 'some value'
        }

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'title': {'value': 'Test Game'}
                }

                mock_metadata = Mock()
                with patch('app.api.curator.UnifiedMetadata', return_value=mock_metadata):
                    response = client.post("/api/curator/update_field", json=request)

        assert response.status_code == 400


# ============================================================================
# Test Suite: Extras
# ============================================================================

class TestExtras:
    """Test suite for listing extra files."""

    def test_get_extras_success(
        self, client, sample_game_folder, mock_config
    ):
        """List extra files successfully."""
        # Create extras directory with files
        extras_dir = sample_game_folder / "Extras"
        extras_dir.mkdir()
        (extras_dir / "manual.pdf").write_text("manual")
        (extras_dir / "artbook.jpg").write_text("artbook")

        with patch('app.api.curator.get_config', return_value=mock_config):
            response = client.get(f"/api/curator/extras/{sample_game_folder.name}")

        assert response.status_code == 200
        data = response.json()
        assert data['total_count'] >= 2
        assert len(data['extras']) >= 2

    def test_get_extras_empty(self, client, sample_game_folder, mock_config):
        """List extras when no extras directory exists."""
        with patch('app.api.curator.get_config', return_value=mock_config):
            response = client.get(f"/api/curator/extras/{sample_game_folder.name}")

        assert response.status_code == 200
        data = response.json()
        assert data['total_count'] == 0


# ============================================================================
# Test Suite: Update Tags (Phase 18.5)
# ============================================================================

class TestUpdateTags:
    """Test suite for updating user-defined tags."""

    def test_update_tags_success(
        self, client, sample_game_folder, mock_config
    ):
        """Successfully update user tags."""
        request = {
            'folder_path': str(sample_game_folder),
            'user_tags': ['favorite', 'completed']
        }

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'title': {'value': 'Test Game'},
                    'user_tags': ['old-tag']
                }

                mock_metadata = Mock()
                mock_metadata.user_tags = ['favorite', 'completed']
                mock_rm.return_value.save_metadata.return_value = True

                with patch('app.api.curator.UnifiedMetadata', return_value=mock_metadata):
                    response = client.patch("/api/curator/games/tags", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True
        assert len(data['user_tags']) == 2


# ============================================================================
# Test Suite: Version Management (Phase 19.6)
# ============================================================================

class TestVersionManagement:
    """Test suite for managing game versions."""

    def test_get_versions(self, client, mock_config):
        """Get all versions for a game."""
        vndb_id = 'v12345'

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'vndb_id': vndb_id,
                    'versions': [
                        {
                            'path': '/path/to/v1',
                            'label': 'Version 1.0',
                            'is_primary': True,
                            'assets': ['game.exe']
                        }
                    ]
                }

                mock_metadata = Mock()
                mock_metadata.vndb_id = vndb_id
                mock_metadata.versions = [
                    Mock(
                        path='/path/to/v1',
                        label='Version 1.0',
                        is_primary=True,
                        assets=['game.exe']
                    )
                ]

                with patch('app.api.curator.UnifiedMetadata', return_value=mock_metadata):
                    response = client.get(f"/api/curator/games/{vndb_id}/versions")

        assert response.status_code == 200
        data = response.json()
        assert data['vndb_id'] == vndb_id

    def test_add_version(self, client, mock_config, tmp_path):
        """Add a new version to a game."""
        vndb_id = 'v12345'
        new_folder = tmp_path / "new_version"
        new_folder.mkdir()

        request = {
            'vndb_id': vndb_id,
            'folder_path': str(new_folder),
            'label': 'Version 2.0'
        }

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'vndb_id': None,
                    'versions': []
                }

                mock_metadata = Mock()
                mock_metadata.versions = []
                mock_metadata.vndb_id = vndb_id
                mock_rm.return_value.save_metadata.return_value = True

                with patch('app.api.curator.create_empty_metadata', return_value={}):
                    with patch('app.api.curator.UnifiedMetadata', return_value=mock_metadata):
                        response = client.post(f"/api/curator/games/{vndb_id}/versions", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True

    def test_set_primary_version(self, client, mock_config):
        """Set a specific version as primary."""
        vndb_id = 'v12345'
        version_id = '/path/to/v2'

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                mock_rm.return_value.load_metadata.return_value = {
                    'vndb_id': vndb_id,
                    'versions': [
                        {'path': '/path/to/v1', 'is_primary': False},
                        {'path': version_id, 'is_primary': False}
                    ]
                }

                mock_metadata = Mock()
                mock_metadata.vndb_id = vndb_id
                mock_metadata.versions = [
                    Mock(path='/path/to/v1', is_primary=False),
                    Mock(path=version_id, is_primary=True)
                ]
                mock_rm.return_value.save_metadata.return_value = True

                with patch('app.api.curator.UnifiedMetadata', return_value=mock_metadata):
                    response = client.patch(
                        f"/api/curator/games/{vndb_id}/versions/{version_id}/primary"
                    )

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True


# ============================================================================
# Test Suite: Smart Identify (Phase 24.0)
# ============================================================================

class TestSmartIdentify:
    """Test suite for smart identification with field locking."""

    def test_smart_identify_with_preserve_locked(
        self, client, sample_game_folder, mock_config
    ):
        """Smart identify preserving locked fields."""
        folder_path = sample_game_folder.name

        request = {
            'folder_path': str(sample_game_folder),
            'vndb_id': 'v12345',
            'preserve_locked': True
        }

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                with patch('app.api.curator.get_database') as mock_db:
                    with patch('app.api.curator.VNDBConnector') as MockVNDB:
                        # Mock existing metadata with locked fields
                        mock_rm.return_value.load_metadata.return_value = {
                            'title': {'value': 'Custom Title', 'locked': True},
                            'developer': {'value': 'Old Dev', 'locked': False},
                            'locked_fields': ['title']
                        }

                        mock_metadata = Mock()
                        mock_metadata.vndb_id = 'v12345'
                        mock_metadata.get_locked_fields.return_value = ['title']
                        mock_rm.return_value.save_metadata.return_value = True

                        # Mock VNDB response
                        mock_vndb = Mock()
                        MockVNDB.return_value = mock_vndb
                        mock_vndb.fetch_metadata = AsyncMock(return_value={
                            'title': 'VNDB Title',
                            'developer': 'VNDB Dev'
                        })

                        with patch('app.api.curator.UnifiedMetadata', return_value=mock_metadata):
                            response = client.post(
                                f"/api/curator/games/{folder_path}/identify",
                                json=request
                            )

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True
        assert 'title' in data['fields_skipped']  # Locked field


# ============================================================================
# Test Suite: Image Selection (Phase 24.0)
# ============================================================================

class TestImageSelection:
    """Test suite for selecting cover/background images."""

    def test_select_cover_image(
        self, client, sample_game_folder, mock_config, tmp_path
    ):
        """Select a cover image successfully."""
        # Create test image
        image_file = sample_game_folder / "cover.jpg"
        image_file.write_text("fake image")

        request = {
            'folder_path': str(sample_game_folder),
            'image_path': 'cover.jpg',
            'image_type': 'cover',
            'create_symlink': False
        }

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_resource_manager') as mock_rm:
                with patch('app.api.curator.get_database') as mock_db:
                    mock_rm.return_value.load_metadata.return_value = {
                        'title': {'value': 'Test Game'},
                        'cover_path': None
                    }

                    mock_metadata = Mock()
                    mock_metadata.cover_path = str(image_file)
                    mock_rm.return_value.save_metadata.return_value = True
                    mock_db.return_value.upsert_game = Mock()

                    with patch('app.api.curator.UnifiedMetadata', return_value=mock_metadata):
                        response = client.post(
                            f"/api/curator/games/{sample_game_folder.name}/images/select",
                            json=request
                        )

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True

    def test_list_images(self, client, sample_game_folder, mock_config, tmp_path):
        """List all images in game folder."""
        # Create test images
        (sample_game_folder / "cover.jpg").write_text("cover")
        (sample_game_folder / "screenshot1.png").write_text("screenshot")
        (sample_game_folder / "extras/art.jpg").mkdir(parents=True)
        (sample_game_folder / "extras/art.jpg/art.jpg").write_text("art")

        with patch('app.api.curator.get_config', return_value=mock_config):
            response = client.get(
                f"/api/curator/games/{sample_game_folder.name}/images"
            )

        assert response.status_code == 200
        data = response.json()
        assert data['total_count'] >= 2
        assert len(data['images']) >= 2


# ============================================================================
# Test Suite: Merge Versions
# ============================================================================

class TestMergeVersions:
    """Test suite for merging game versions."""

    def test_merge_versions_success(self, client, mock_config):
        """Merge all versions of a game successfully."""
        vndb_id = 'v12345'

        request = {
            'vndb_id': vndb_id,
            'primary_folder': None
        }

        mock_curator = Mock()
        mock_result = {
            'success': True,
            'vndb_id': vndb_id,
            'folders_found': 3,
            'folders_updated': 3,
            'message': 'Merged 3 folders'
        }
        mock_curator.merge_versions.return_value = mock_result

        with patch('app.api.curator.get_config', return_value=mock_config):
            with patch('app.api.curator.get_curator', return_value=mock_curator):
                response = client.post("/api/curator/merge_versions", json=request)

        assert response.status_code == 200
        data = response.json()
        assert data['success'] is True
        assert data['folders_updated'] == 3


# ============================================================================
# Test Suite: Error Handling
# ============================================================================

class TestCuratorErrorHandling:
    """Test suite for error handling."""

    def test_invalid_folder_path(self, client, mock_config):
        """Request with invalid folder path."""
        request = {
            'folder_path': '../../../etc',
            'vndb_id': 'v12345',
            'fetch_metadata': False
        }

        with patch('app.api.curator.get_config', return_value=mock_config):
            response = client.post("/api/curator/identify", json=request)

        assert response.status_code in [400, 404, 500]

    def test_missing_required_field(self, client):
        """Request missing required field."""
        request = {
            'folder_path': '/some/path'
            # Missing vndb_id
        }

        response = client.post("/api/curator/identify", json=request)

        assert response.status_code == 422
