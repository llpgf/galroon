"""
Unit Tests - API path safety helpers.
"""

import tempfile
from pathlib import Path

import pytest
from fastapi import HTTPException

from app.api.curator import _resolve_child_path, _resolve_folder_path
from app.api.connectors import _resolve_game_path
from app.api.utilities import _resolve_library_path


def _make_dirs():
    root = tempfile.TemporaryDirectory()
    other = tempfile.TemporaryDirectory()
    return root, other


def test_resolve_library_path_blocks_external():
    root, other = _make_dirs()
    try:
        library_root = Path(root.name)
        outside = Path(other.name)

        with pytest.raises(HTTPException):
            _resolve_library_path(str(outside), library_root, allow_external=False)
    finally:
        root.cleanup()
        other.cleanup()


def test_resolve_library_path_allows_external():
    root, other = _make_dirs()
    try:
        library_root = Path(root.name)
        outside = Path(other.name)

        resolved = _resolve_library_path(str(outside), library_root, allow_external=True)
        assert resolved == outside
    finally:
        root.cleanup()
        other.cleanup()


def test_resolve_folder_path_relative():
    root, _other = _make_dirs()
    try:
        library_root = Path(root.name)
        (library_root / "game").mkdir()

        resolved = _resolve_folder_path("game", library_root)
        assert resolved == library_root / "game"
    finally:
        root.cleanup()
        _other.cleanup()


def test_resolve_folder_path_blocks_external():
    root, other = _make_dirs()
    try:
        library_root = Path(root.name)
        outside = Path(other.name)

        with pytest.raises(HTTPException):
            _resolve_folder_path(str(outside), library_root)
    finally:
        root.cleanup()
        other.cleanup()


def test_resolve_child_path_blocks_escape():
    root, _other = _make_dirs()
    try:
        library_root = Path(root.name)
        parent = library_root / "game"
        parent.mkdir()

        with pytest.raises(HTTPException):
            _resolve_child_path(parent, "../escape.txt")
    finally:
        root.cleanup()
        _other.cleanup()


def test_resolve_game_path_blocks_external():
    root, other = _make_dirs()
    try:
        library_root = Path(root.name)
        outside = Path(other.name)

        with pytest.raises(HTTPException):
            _resolve_game_path(str(outside), library_root)
    finally:
        root.cleanup()
        other.cleanup()
