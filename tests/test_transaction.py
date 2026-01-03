"""
Unit tests for Transaction Engine.

Tests FSM-based file operations with rollback capabilities.
"""

from pathlib import Path

import pytest

from app.core import JournalManager, Transaction, TransactionState
from app.core.transaction import TransactionExecutionError, TransactionValidationError


class TestTransactionMkdir:
    """Test suite for mkdir transaction operations."""

    def test_mkdir_prepare_and_commit(self, tmp_path):
        """Successful mkdir transaction should create directory."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        new_dir = library / "new_game"
        tx.prepare("mkdir", new_dir).commit()

        assert new_dir.exists()
        assert new_dir.is_dir()
        assert tx.get_state() == TransactionState.COMMITTED

    def test_mkdir_rollback_removes_directory(self, tmp_path):
        """Rollback of mkdir should remove the created directory."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        new_dir = library / "new_game"
        tx.prepare("mkdir", new_dir)
        tx.commit()  # Create directory
        tx.rollback()  # Remove it

        assert not new_dir.exists()
        assert tx.get_state() == TransactionState.ROLLED_BACK

    def test_mkdir_fails_if_exists(self, tmp_path):
        """Mkdir should fail if directory already exists."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        existing_dir = library / "existing"
        existing_dir.mkdir()

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        with pytest.raises(TransactionValidationError, match="already exists"):
            tx.prepare("mkdir", existing_dir)

    def test_mkdir_creates_journal_entry(self, tmp_path):
        """Mkdir should create a journal entry."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        new_dir = library / "new_game"
        tx.prepare("mkdir", new_dir)

        # Check journal has entry
        entries = journal.read_all()
        assert len(entries) == 1
        assert entries[0].op == "mkdir"
        assert entries[0].state == "prepared"


class TestTransactionRename:
    """Test suite for rename transaction operations."""

    def test_rename_file(self, tmp_path):
        """Successful rename transaction should move file."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        old_file = library / "old_name.txt"
        new_file = library / "new_name.txt"
        old_file.write_text("test content")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        tx.prepare("rename", old_file, new_file).commit()

        assert not old_file.exists()
        assert new_file.exists()
        assert new_file.read_text() == "test content"
        assert tx.get_state() == TransactionState.COMMITTED

    def test_rename_directory(self, tmp_path):
        """Rename should work for directories."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        old_dir = library / "old_game"
        new_dir = library / "new_game"
        old_dir.mkdir()
        (old_dir / "game.exe").write_text("binary")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        tx.prepare("rename", old_dir, new_dir).commit()

        assert not old_dir.exists()
        assert new_dir.exists()
        assert (new_dir / "game.exe").exists()

    def test_rename_rollback_reverses_move(self, tmp_path):
        """Rollback of rename should move file back."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        old_file = library / "old.txt"
        new_file = library / "new.txt"
        old_file.write_text("content")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        tx.prepare("rename", old_file, new_file)
        tx.commit()
        tx.rollback()

        assert old_file.exists()
        assert not new_file.exists()
        assert old_file.read_text() == "content"

    def test_rename_fails_if_dest_exists(self, tmp_path):
        """Rename should fail if destination already exists."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        src = library / "src.txt"
        dst = library / "dst.txt"
        src.write_text("source")
        dst.write_text("dest")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        with pytest.raises(TransactionValidationError, match="already exists"):
            tx.prepare("rename", src, dst)


class TestTransactionCopy:
    """Test suite for copy transaction operations."""

    def test_copy_file(self, tmp_path):
        """Successful copy transaction should duplicate file."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        src_file = library / "original.txt"
        dst_file = library / "copy.txt"
        src_file.write_text("test content")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        tx.prepare("copy", src_file, dst_file).commit()

        assert src_file.exists()
        assert dst_file.exists()
        assert src_file.read_text() == dst_file.read_text()
        assert tx.get_state() == TransactionState.COMMITTED

    def test_copy_directory(self, tmp_path):
        """Copy should work for directories."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        src_dir = library / "original_game"
        dst_dir = library / "copy_game"
        src_dir.mkdir()
        (src_dir / "game.exe").write_text("binary")
        (src_dir / "data.txt").write_text("save")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        tx.prepare("copy", src_dir, dst_dir).commit()

        assert src_dir.exists()
        assert dst_dir.exists()
        assert (dst_dir / "game.exe").read_text() == "binary"
        assert (dst_dir / "data.txt").read_text() == "save"

    def test_copy_rollback_removes_copy(self, tmp_path):
        """Rollback of copy should remove the copied files."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        src_file = library / "original.txt"
        dst_file = library / "copy.txt"
        src_file.write_text("content")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        tx.prepare("copy", src_file, dst_file)
        tx.commit()
        tx.rollback()

        assert src_file.exists()
        assert not dst_file.exists()

    def test_copy_fails_if_dest_exists(self, tmp_path):
        """Copy should fail if destination already exists."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        src = library / "src.txt"
        dst = library / "dst.txt"
        src.write_text("source")
        dst.write_text("dest")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        with pytest.raises(TransactionValidationError, match="already exists"):
            tx.prepare("copy", src, dst)


class TestTransactionDelete:
    """Test suite for delete transaction operations."""

    def test_delete_file(self, tmp_path):
        """Successful delete transaction should remove file."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        target = library / "to_delete.txt"
        target.write_text("will be deleted")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        tx.prepare("delete", target).commit()

        assert not target.exists()
        assert tx.get_state() == TransactionState.COMMITTED

    def test_delete_directory(self, tmp_path):
        """Delete should work for directories."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        target_dir = library / "game_folder"
        target_dir.mkdir()
        (target_dir / "game.exe").write_text("binary")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        tx.prepare("delete", target_dir).commit()

        assert not target_dir.exists()

    def test_delete_rollback_logs_warning(self, tmp_path, caplog):
        """Rollback of delete should log warning (cannot restore)."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        target = library / "deleted.txt"
        target.write_text("gone forever")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        tx.prepare("delete", target)
        tx.commit()
        tx.rollback()

        # File is still deleted (rollback can't restore it)
        assert not target.exists()
        assert tx.get_state() == TransactionState.ROLLED_BACK

    def test_delete_fails_if_not_exists(self, tmp_path):
        """Delete should fail if source doesn't exist."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        nonexistent = library / "does_not_exist.txt"

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        with pytest.raises(TransactionValidationError, match="does not exist"):
            tx.prepare("delete", nonexistent)


class TestTransactionPathSafety:
    """Test suite for path safety validation in transactions."""

    def test_rejects_path_outside_library(self, tmp_path):
        """Transaction should reject paths outside library root."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        outside = tmp_path / "outside"
        config.mkdir()
        library.mkdir()
        outside.mkdir()

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        # Try to mkdir outside library
        outside_dir = outside / "escaped"
        with pytest.raises(TransactionValidationError, match="Path validation failed"):
            tx.prepare("mkdir", outside_dir)

    def test_rejects_symlink_escape(self, tmp_path):
        """Transaction should reject symlink paths outside library."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        outside = tmp_path / "outside"
        config.mkdir()
        library.mkdir()
        outside.mkdir()

        # Create symlink inside library pointing outside
        link = library / "escape_link"
        try:
            link.symlink_to(outside)
        except OSError:
            pytest.skip("Symlink creation not supported")

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        # Try to mkdir through symlink
        outside_mkdir = link / "escaped"
        with pytest.raises(TransactionValidationError, match="Path validation failed"):
            tx.prepare("mkdir", outside_mkdir)

    def test_rejects_parent_traversal(self, tmp_path):
        """Transaction should reject parent directory traversal."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        # Try to use .. to escape
        escape_path = library / ".." / "config"
        with pytest.raises(TransactionValidationError, match="Path validation failed"):
            tx.prepare("mkdir", escape_path)


class TestTransactionFSM:
    """Test suite for transaction state machine."""

    def test_cannot_commit_without_prepare(self, tmp_path):
        """Cannot commit without preparing first."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        with pytest.raises(Exception, match="No entry to commit"):
            tx.commit()

    def test_cannot_prepare_twice(self, tmp_path):
        """Cannot prepare a transaction twice."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        new_dir = library / "new"
        tx.prepare("mkdir", new_dir)

        with pytest.raises(Exception):
            tx.prepare("mkdir", new_dir)

    def test_journal_entry_state_updates(self, tmp_path):
        """Journal entry should reflect state changes."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        journal = JournalManager(config)
        tx = Transaction(journal, library)

        new_dir = library / "new"
        tx.prepare("mkdir", new_dir)

        # Should have prepared entry
        entries = journal.read_all()
        assert len(entries) == 1
        assert entries[0].state == "prepared"
        tx_id = entries[0].tx_id

        tx.commit()

        # Should have committed entry
        entries = journal.read_all()
        committed_entries = [e for e in entries if e.tx_id == tx_id and e.state == "committed"]
        assert len(committed_entries) == 1

        tx.rollback()

        # Should have rolled_back entry
        entries = journal.read_all()
        rolled_back_entries = [e for e in entries if e.tx_id == tx_id and e.state == "rolled_back"]
        assert len(rolled_back_entries) == 1


class TestTransactionMethodChaining:
    """Test suite for method chaining interface."""

    def test_fluent_interface(self, tmp_path):
        """Transaction should support method chaining."""
        config = tmp_path / "config"
        library = tmp_path / "library"
        config.mkdir()
        library.mkdir()

        journal = JournalManager(config)

        # Chain prepare and commit
        new_dir = library / "new"
        tx = Transaction(journal, library).prepare("mkdir", new_dir).commit()

        assert new_dir.exists()
        assert tx.get_state() == TransactionState.COMMITTED
