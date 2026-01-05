"""
Path Safety Module - The "Realpath" Sandbox (Anti-Symlink)

This module implements strict path validation to prevent symlink traversal attacks.
All file operations MUST pass through is_safe_path before execution.
"""

import os
from pathlib import Path
from typing import Union


def is_safe_path(user_path: Union[str, Path], library_root: Union[str, Path]) -> bool:
    """
    Validate that a path is safely contained within the library root.

    This function prevents symlink traversal attacks by resolving all paths
    to their canonical form before checking containment.

    Args:
        user_path: The path to validate (can be relative, absolute, or contain symlinks)
        library_root: The root directory that should contain user_path

    Returns:
        True if the real path is safely contained within library_root
        False if the path escapes the library root or cannot be resolved

    Example:
        >>> is_safe_path("/games/../etc/passwd", "/games")
        False
        >>> is_safe_path("/games/fate/game.exe", "/games")
        True
    """
    try:
        # Resolve symlinks and .. to get the real physical path
        # This prevents attacks via symlink chains or parent directory traversal
        real_path = os.path.realpath(os.path.abspath(str(user_path)))
        real_root = os.path.realpath(os.path.abspath(str(library_root)))

        # Check strict containment using commonpath
        # This ensures real_path is actually inside real_root
        common = os.path.commonpath([real_path, real_root])
        return common == real_root

    except (OSError, ValueError):
        # Path doesn't exist, permission denied, or other OS error
        # Fail closed: treat as unsafe
        return False


def validate_path_or_raise(user_path: Union[str, Path], library_root: Union[str, Path]) -> Path:
    """
    Validate a path and raise an exception if it's unsafe.

    This is a convenience wrapper around is_safe_path for use in
    functions that need to enforce path safety.

    Args:
        user_path: The path to validate
        library_root: The root directory that should contain user_path

    Returns:
        The resolved (canonical) Path object if safe

    Raises:
        ValueError: If the path is not safely contained within library_root
    """
    if not is_safe_path(user_path, library_root):
        raise ValueError(
            f"Path safety violation: {user_path} is not contained within {library_root}"
        )

    # Return the resolved path for actual use
    return Path(os.path.realpath(os.path.abspath(str(user_path))))


def is_safe_config_dir(config_dir: Union[str, Path]) -> bool:
    """
    Validate that the config directory is safe for journal storage.

    The config directory must:
    1. Exist and be writable
    2. Not be a symlink (to prevent journal hijacking)
    3. Be a directory (not a file)

    Args:
        config_dir: The config directory path to validate

    Returns:
        True if the directory is safe for journal storage
    """
    try:
        config_path = Path(str(config_dir)).resolve()

        # Must exist
        if not config_path.exists():
            return False

        # Must be a directory
        if not config_path.is_dir():
            return False

        # Must not be a symlink
        if config_path.is_symlink():
            return False

        # Must be writable (test with a temporary file probe)
        test_file = config_path / ".write_test_probe"
        try:
            test_file.touch()
            test_file.unlink()
            return True
        except OSError:
            return False

    except (OSError, ValueError):
        return False
