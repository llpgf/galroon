"""Read-only clone of an existing generated scale fixture into a fresh isolated state."""
from pathlib import Path
import sqlite3
import sys

source = Path(sys.argv[1]).resolve(strict=True)
out = Path(sys.argv[2]).resolve()
assert not out.exists(), 'Choose a fresh output directory'
state = out / 'state'
state.mkdir(parents=True)
with sqlite3.connect(source.as_uri() + '?mode=ro', uri=True) as original:
    with sqlite3.connect(state / 'library.sqlite') as copied:
        original.backup(copied)
        assert copied.execute('SELECT count(*) FROM works').fetchone()[0] == 100000
        for (root_id,) in copied.execute('SELECT id FROM roots').fetchall():
            root = out / 'generated-empty' / root_id
            assert root.resolve().is_relative_to(out)
            root.mkdir(parents=True)
            copied.execute('UPDATE roots SET path=? WHERE id=?', (str(root), root_id))
            copied.execute("INSERT INTO source_watch(root_id,path,enabled,status,needs_scan) VALUES(?,?,0,'disabled',0) ON CONFLICT(root_id) DO UPDATE SET path=excluded.path,enabled=0,status='disabled',needs_scan=0", (root_id, str(root)))
        copied.commit()
print('Prepared generated 100000-work copy; original opened read-only; watchers disabled')
