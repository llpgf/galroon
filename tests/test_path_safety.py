"""
Unit tests for path safety functions.

Tests the realpath sandbox that prevents symlink traversal attacks.
"""

import os
import tempfile
from pathlib import Path

import pytest

from app.core.path_safety import is_safe_path, is_safe_config_dir, validate_path_or_raise


class TestSafePath:
    """Test suite for is_safe_path function."""

    def test_safe_path_within_root(self, tmp_path):
        """Normal paths within root should be safe."""
        assert is_safe_path(tmp_path / "subdir" / "file.txt", tmp_path)
        assert is_safe_path(tmp_path / "game.exe", tmp_path)
        assert is_safe_path(tmp_path, tmp_path)

    def test_parent_directory_traversal(self, tmp_path):
        """Parent directory traversal (..) should be blocked."""
        library = tmp_path / "library"
        library.mkdir()

        # Direct parent traversal
        assert not is_safe_path(tmp_path / "etc" / "passwd", library)

        # Traversal via .. in path
        assert not is_safe_path(library / ".." / "etc", library)

    def test_symlink_traversal(self, tmp_path):
        """Symlinks pointing outside root should be blocked."""
        library = tmp_path / "library"
        outside = tmp_path / "outside"
        library.mkdir()
        outside.mkdir()

        # Create a symlink inside library pointing outside
        link = library / "escape_link"
        try:
            link.symlink_to(outside)
        except OSError:
            pytest.skip("Symlink creation not supported")

        # The symlink itself should be safe (it's inside library)
        assert is_safe_path(link, library)

        # But accessing files through the symlink should be unsafe
        outside_file = outside / "secret.txt"
        outside_file.touch()

        # Path through symlink should be blocked
        assert not is_safe_path(link / "secret.txt", library)

    def test_symlink_chain(self, tmp_path):
        """Chained symlinks should still be contained."""
        library = tmp_path / "library"
        library.mkdir()

        # Create chain: link1 -> link2 -> actual_file
        actual_file = library / "actual.txt"
        actual_file.touch()

        link2 = library / "link2"
        link1 = library / "link1"

        try:
            link2.symlink_to(actual_file)
            link1.symlink_to(link2)
        except OSError:
            pytest.skip("Symlink creation not supported")

        # All should be safe (they resolve to file inside library)
        assert is_safe_path(link1, library)
        assert is_safe_path(link2, library)

    def test_absolute_vs_relative_paths(self, tmp_path):
        """Both absolute and relative paths should work."""
        library = tmp_path / "library"
        library.mkdir()

        # Absolute path
        abs_path = library / "game.exe"
        assert is_safe_path(abs_path, library)

        # Relative path (requires context of cwd)
        # Change to library dir
        old_cwd = os.getcwd()
        try:
            os.chdir(library)
            Path("game.exe").touch()
            assert is_safe_path("game.exe", library)
            assert is_safe_path(Path("game.exe"), library)
        finally:
            os.chdir(old_cwd)

    def test_nonexistent_paths(self, tmp_path):
        """Nonexistent paths should still be validated correctly."""
        library = tmp_path / "library"
        library.mkdir()

        # Safe nonexistent path
        assert is_safe_path(library / "future" / "file.txt", library)

        # Unsafe nonexistent path
        assert not is_safe_path(library / ".." / "etc", library)

    def test_empty_and_edge_cases(self, tmp_path):
        """Edge cases should be handled safely."""
        library = tmp_path / "library"
        library.mkdir()

        # Empty string
        assert not is_safe_path("", library)

        # Root path
        if os.name != 'nt':  # Unix only
            assert not is_safe_path("/", library)


class TestValidatePathOrRaise:
    """Test suite for validate_path_or_raise function."""

    def test_safe_path_returns_resolved(self, tmp_path):
        """Safe paths should return resolved Path object."""
        library = tmp_path / "library"
        library.mkdir()

        result = validate_path_or_raise(library / "game.exe", library)
        assert result == library.resolve() / "game.exe"

    def test_unsafe_path_raises(self, tmp_path):
        """Unsafe paths should raise ValueError."""
        library = tmp_path / "library"
        library.mkdir()

        with pytest.raises(ValueError, match="Path safety violation"):
            validate_path_or_raise(tmp_path / "etc" / "passwd", library)


class TestSafeConfigDir:
    """Test suite for is_safe_config_dir function."""

    def test_valid_config_directory(self, tmp_path):
        """Normal writable directory should be safe."""
        config = tmp_path / "config"
        config.mkdir()

        assert is_safe_config_dir(config)

    def test_nonexistent_directory(self, tmp_path):
        """Nonexistent directory should be unsafe."""
        config = tmp_path / "nonexistent"

        assert not is_safe_config_dir(config)

    def test_file_not_directory(self, tmp_path):
        """A file should not be treated as safe config dir."""
        config = tmp_path / "not_a_dir"
        config.touch()

        assert not is_safe_config_dir(config)

    def test_symlink_directory(self, tmp_path):
        """Symlink directory should be unsafe."""
        real_dir = tmp_path / "real_config"
        real_dir.mkdir()

        link = tmp_path / "config_link"
        try:
            link.symlink_to(real_dir)
        except OSError:
            pytest.skip("Symlink creation not supported")

        assert not is_safe_config_dir(link)

    def test_read_only_directory(self, tmp_path):
        """Read-only directory should be unsafe."""
        # Skip on Windows as chmod behaves differently
        if os.name == 'nt':
            pytest.skip("chmod read-only test not reliable on Windows")

        config = tmp_path / "readonly"
        config.mkdir()

        # Make directory read-only
        try:
            os.chmod(config, 0o444)

            # Should be unsafe (not writable)
            assert not is_safe_config_dir(config)

        except OSError:
            pytest.skip("chmod not supported")
        finally:
            # Restore permissions for cleanup
            try:
                os.chmod(config, 0o755)
            except OSError:
                pass

    def test_writable_test_file_cleanup(self, tmp_path):
        """The write test probe should be cleaned up."""
        config = tmp_path / "config"
        config.mkdir()

        assert is_safe_config_dir(config)

        # Probe file should not exist
        assert not (config / ".write_test_probe").exists()
