"""
Unit tests for Journal Manager.

Tests atomic journaling, recovery logic, and transaction management.
"""

import json
import time
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from app.core.journal import JournalManager
from app.models.journal import JournalEntry


class TestJournalEntry:
    """Test suite for JournalEntry model."""

    def test_create_entry(self):
        """JournalEntry should be created with proper defaults."""
        entry = JournalEntry(
            op="rename",
            src="/games/fate",
            dest="/games/Fate",
            state="prepared",
            timeout_at=time.time() + 300
        )

        assert entry.tx_id is not None
        assert entry.op == "rename"
        assert entry.state == "prepared"

    def test_stale_detection(self):
        """is_stale should correctly identify stale transactions."""
        past_timeout = time.time() - 10
        future_timeout = time.time() + 300

        stale_entry = JournalEntry(
            op="delete",
            src="/games/old",
            state="prepared",
            timeout_at=past_timeout
        )

        fresh_entry = JournalEntry(
            op="mkdir",
            src="/games/new",
            state="prepared",
            timeout_at=future_timeout
        )

        assert stale_entry.is_stale() is True
        assert fresh_entry.is_stale() is False

    def test_serialization(self):
        """to_journal_line should produce valid JSON with newline."""
        entry = JournalEntry(
            op="copy",
            src="/games/src",
            dest="/games/dest",
            state="committed",
            timeout_at=time.time() + 300
        )

        line = entry.to_journal_line()

        # Should be valid JSON
        data = json.loads(line)
        assert data["op"] == "copy"
        assert data["state"] == "committed"

        # Should end with newline
        assert line.endswith("\n")


class TestJournalManager:
    """Test suite for JournalManager."""

    def test_init_creates_journal_file(self, tmp_path):
        """JournalManager should create journal.jsonl if it doesn't exist."""
        config = tmp_path / "config"
        config.mkdir()

        journal_path = config / "journal.jsonl"
        assert not journal_path.exists()

        jm = JournalManager(config)

        assert journal_path.exists()

    def test_refuses_unsafe_config_dir(self, tmp_path):
        """JournalManager should refuse to start with unsafe config dir."""
        nonexistent = tmp_path / "nonexistent"

        with pytest.raises(RuntimeError, match="Journal sandbox violation"):
            JournalManager(nonexistent)

    def test_append_write(self, tmp_path):
        """Append should write entry to journal file."""
        config = tmp_path / "config"
        config.mkdir()

        jm = JournalManager(config)

        entry = JournalEntry(
            op="rename",
            src="/games/fate",
            dest="/games/Fate",
            state="prepared",
            timeout_at=time.time() + 300
        )

        jm.append(entry)

        # Read back and verify
        lines = (config / "journal.jsonl").read_text().strip().split("\n")
        assert len(lines) == 1

        data = json.loads(lines[0])
        assert data["op"] == "rename"
        assert data["src"] == "/games/fate"

    def test_read_all_entries(self, tmp_path):
        """read_all should parse all entries correctly."""
        config = tmp_path / "config"
        config.mkdir()

        jm = JournalManager(config)

        # Add multiple entries
        for i in range(3):
            entry = JournalEntry(
                op="mkdir" if i % 2 == 0 else "delete",
                src=f"/games/test{i}",
                state="committed",
                timeout_at=time.time() + 300
            )
            jm.append(entry)

        # Read back
        entries = jm.read_all()
        assert len(entries) == 3
        assert entries[0].op == "mkdir"
        assert entries[1].op == "delete"
        assert entries[2].op == "mkdir"

    def test_invalid_json_is_skipped(self, tmp_path):
        """Invalid JSON lines in journal should be logged but not crash."""
        config = tmp_path / "config"
        config.mkdir()

        # Create journal with some invalid data
        journal_file = config / "journal.jsonl"
        journal_file.write_text(
            '{"op": "rename", "src": "/games/a", "dest": "/games/b", "state": "committed", "timeout_at": 123}\n'
            "this is not json\n"
            '{"op": "delete", "src": "/games/c", "state": "committed", "timeout_at": 456}\n'
        )

        jm = JournalManager(config)
        entries = jm.read_all()

        # Should skip the invalid line
        assert len(entries) == 2
        assert entries[0].op == "rename"
        assert entries[1].op == "delete"

    def test_get_incomplete_transactions(self, tmp_path):
        """Should only return entries with state='prepared'."""
        config = tmp_path / "config"
        config.mkdir()

        jm = JournalManager(config)

        # Add various states
        states = ["prepared", "committed", "prepared", "rolled_back", "failed"]
        for state in states:
            entry = JournalEntry(
                op="rename",
                src=f"/games/{state}",
                state=state,
                timeout_at=time.time() + 300
            )
            jm.append(entry)

        incomplete = jm.get_incomplete_transactions()
        assert len(incomplete) == 2
        assert all(e.state == "prepared" for e in incomplete)

    def test_get_stale_transactions(self, tmp_path):
        """Should identify prepared transactions past timeout."""
        config = tmp_path / "config"
        config.mkdir()

        jm = JournalManager(config)

        # Fresh transaction
        fresh = JournalEntry(
            op="rename",
            src="/games/fresh",
            state="prepared",
            timeout_at=time.time() + 300
        )
        jm.append(fresh)

        # Stale transaction
        stale = JournalEntry(
            op="delete",
            src="/games/stale",
            state="prepared",
            timeout_at=time.time() - 10
        )
        jm.append(stale)

        stale_transactions = jm.get_stale_transactions()
        assert len(stale_transactions) == 1
        assert stale_transactions[0].tx_id == stale.tx_id

    def test_recovery_without_handler(self, tmp_path):
        """Recovery should identify stale/active without rolling back."""
        config = tmp_path / "config"
        config.mkdir()

        jm = JournalManager(config)

        # Add stale and fresh transactions
        stale = JournalEntry(
            op="rename",
            src="/games/stale",
            state="prepared",
            timeout_at=time.time() - 10
        )
        jm.append(stale)

        fresh = JournalEntry(
            op="mkdir",
            src="/games/fresh",
            state="prepared",
            timeout_at=time.time() + 300
        )
        jm.append(fresh)

        result = jm.recover()

        assert len(result['stale']) == 1
        assert len(result['active']) == 1
        assert result['stale'][0].tx_id == stale.tx_id
        assert result['active'][0].tx_id == fresh.tx_id

    def test_recovery_with_rollback_handler(self, tmp_path):
        """Recovery should call rollback handler for stale transactions."""
        config = tmp_path / "config"
        config.mkdir()

        jm = JournalManager(config)

        stale = JournalEntry(
            op="delete",
            src="/games/stale",
            state="prepared",
            timeout_at=time.time() - 10
        )
        jm.append(stale)

        rollback_mock = MagicMock(return_value=True)
        result = jm.recover(rollback_handler=rollback_mock)

        # Rollback should be called once
        rollback_mock.assert_called_once()
        assert rollback_mock.call_args[0][0].tx_id == stale.tx_id

    def test_journal_persistence(self, tmp_path):
        """Journal should persist across JournalManager instances."""
        config = tmp_path / "config"
        config.mkdir()

        # First instance: write entry
        jm1 = JournalManager(config)
        entry = JournalEntry(
            op="copy",
            src="/games/src",
            dest="/games/dest",
            state="prepared",
            timeout_at=time.time() + 300
        )
        jm1.append(entry)

        # Second instance: read entry
        jm2 = JournalManager(config)
        entries = jm2.read_all()

        assert len(entries) == 1
        assert entries[0].tx_id == entry.tx_id
