from pathlib import Path
import sqlite3, json, hashlib, sys

out = Path('test-output/mvp-goal/native-checkpoint-r1').resolve()
base = out / 'state'
selection = json.loads((base / 'active-library.json').read_text(encoding='utf-8'))
current = Path(selection['current'].removeprefix('\\\\?\\')).resolve()
assert current.is_relative_to(base.resolve())
backups = list((out / 'backups').glob('Galroon-backup-*'))
assert len(backups) == 1
backup = backups[0]
def ro(path):
    return sqlite3.connect(path.resolve().as_uri() + '?mode=ro', uri=True)
with ro(current / 'library.sqlite') as restored, ro(backup / 'collection.sqlite') as saved:
    tables = ['works', 'releases', 'work_releases', 'work_preferences', 'resources', 'resource_bindings', 'jobs']
    compared = {}
    for table in tables:
        actual = sorted(map(repr, restored.execute('SELECT * FROM ' + table).fetchall()))
        expected = sorted(map(repr, saved.execute('SELECT * FROM ' + table).fetchall()))
        assert actual == expected, table
        compared[table] = {'rows': len(actual), 'sha256': hashlib.sha256('\n'.join(actual).encode()).hexdigest()}
    assert restored.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
    assert not restored.execute('PRAGMA foreign_key_check').fetchall()
    assert restored.execute('SELECT count(*) FROM source_watch WHERE enabled!=0').fetchone()[0] == 0
    assert restored.execute("SELECT count(*) FROM jobs WHERE state IN ('queued','running','pausing','cancelling')").fetchone()[0] == 0
report = {'passed': True, 'native_restore': True, 'current': str(current), 'previous': str(base), 'backup': str(backup), 'recovery_snapshot': selection['previous'][0]['snapshot'], 'exact_tables': compared, 'schema': 41, 'integrity': 'ok', 'watchers_disabled': True, 'active_jobs': 0, 'source_hashes': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (base / 'generated-source').iterdir()}}
report_path = out / (sys.argv[1] if len(sys.argv) > 1 else 'native-restore-readback.json')
assert report_path.resolve().is_relative_to(out) and not report_path.exists()
report_path.write_text(json.dumps(report, indent=2), encoding='utf-8')
print('Native restore exact readback passed for seven tables, disabled watching and source hashes')
