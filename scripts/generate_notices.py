"""Build a conservative Windows dependency notice inventory from locked local sources.

Run with Python 3 after npm ci and cargo fetch. Network is used only with
--fetch-missing, to recover omitted upstream license files at the crate's recorded
Git commit. Cached upstream files and their hashes remain reviewable in source.
This inventory includes build dependencies; it is not a binary composition claim.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import shutil
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
PREFIXES = ('license', 'licence', 'copying', 'notice', 'copyright', 'unlicense', 'ofl')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def license_files(directory):
    return sorted(p for p in directory.rglob('*') if p.is_file()
                  and p.name.lower().startswith(PREFIXES)
                  and p.suffix.lower() not in {'.rs', '.js', '.ts', '.json', '.png', '.svg'})


def fetch_json(url):
    req = urllib.request.Request(url, headers={'User-Agent': 'Galroon-notice-inventory'})
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)


def upstream_files(package, directory, allow_fetch):
    vcs_file = directory / '.cargo_vcs_info.json'
    if not vcs_file.exists():
        return []
    commit = json.loads(vcs_file.read_text(encoding='utf-8'))['git']['sha1']
    repository = (package.get('repository') or '').rstrip('/').removesuffix('.git')
    match = re.fullmatch(r'https://github.com/([^/]+/[^/]+)', repository)
    if not match:
        return []
    cache = ROOT / 'third-party' / 'upstream' / f"{package['name']}-{package['version']}"
    provenance = cache / 'provenance.json'
    if not provenance.exists() and allow_fetch:
        tree = fetch_json(f'https://api.github.com/repos/{match[1]}/git/trees/{commit}')
        files = [x for x in tree['tree'] if x['type'] == 'blob'
                 and x['path'].lower().startswith(PREFIXES)]
        cache.mkdir(parents=True, exist_ok=True)
        saved = []
        for item in files:
            url = f'https://raw.githubusercontent.com/{match[1]}/{commit}/{item["path"]}'
            with urllib.request.urlopen(url, timeout=30) as response:
                data = response.read()
            (cache / item['path']).write_bytes(data)
            saved.append({'file': item['path'], 'url': url, 'sha256': digest(data)})
        provenance.write_text(json.dumps({'repository': repository, 'commit': commit,
                                         'files': saved}, indent=2) + '\n', encoding='utf-8')
    if not provenance.exists():
        return []
    record = json.loads(provenance.read_text(encoding='utf-8'))
    assert record['commit'] == commit and record['repository'] == repository
    results = []
    for item in record['files']:
        path = cache / item['file']
        assert digest(path.read_bytes()) == item['sha256'], path
        results.append((path, item['url']))
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fetch-missing', action='store_true')
    args = parser.parse_args()
    raw = subprocess.check_output(['cargo', 'metadata', '--locked', '--offline',
                                   '--format-version', '1', '--filter-platform',
                                   'x86_64-pc-windows-msvc'], cwd=ROOT)
    metadata = json.loads(raw)
    packages = {p['id']: p for p in metadata['packages']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    seen, todo = set(), list(metadata['workspace_members'])
    while todo:
        key = todo.pop()
        if key in seen:
            continue
        seen.add(key)
        todo.extend(dep['pkg'] for dep in nodes[key]['deps']
                    if any(kind['kind'] != 'dev' for kind in dep['dep_kinds']))
    entries, missing, sections = [], [], []

    def add(ecosystem, name, version, declared, repository, files):
        record = {'ecosystem': ecosystem, 'name': name, 'version': version,
                  'declared_license': declared, 'repository': repository, 'notices': []}
        if ecosystem == 'Cargo':
            record['source_archive'] = f'https://crates.io/api/v1/crates/{name}/{version}/download'
        text = [f'\n{"=" * 78}\n{ecosystem}: {name} {version}',
                f'Declared license: {declared or "not declared"}', f'Upstream: {repository or "not recorded"}']
        if not files:
            missing.append(f'{ecosystem}:{name}@{version}')
        for path, source in files:
            data = path.read_bytes()
            record['notices'].append({'source': source, 'sha256': digest(data)})
            text.extend([f'\n--- {source} ---\n', data.decode('utf-8-sig', errors='replace')])
        entries.append(record)
        sections.append('\n'.join(text))

    for key in sorted(seen):
        package = packages[key]
        if package['source'] is None:
            continue
        directory = Path(package['manifest_path']).parent
        files = [(p, f"crate:{package['name']}@{package['version']}/{p.relative_to(directory).as_posix()}")
                 for p in license_files(directory)]
        if not files:
            files = upstream_files(package, directory, args.fetch_missing)
        add('Cargo', package['name'], package['version'], package['license'], package['repository'], files)

    lock = json.loads((ROOT / 'package-lock.json').read_text(encoding='utf-8'))
    for location, package in sorted(lock['packages'].items()):
        if not location or package.get('dev'):
            continue
        directory = ROOT / location
        info = json.loads((directory / 'package.json').read_text(encoding='utf-8'))
        assert info['version'] == package['version'], location
        repo = info.get('repository')
        if isinstance(repo, dict):
            repo = repo.get('url')
        files = [(p, f"npm:{info['name']}@{info['version']}/{p.relative_to(directory).as_posix()}")
                 for p in license_files(directory) if 'node_modules' not in p.relative_to(directory).parts]
        add('npm', info['name'], info['version'], info.get('license'), repo, files)

    add('runtime', '7-Zip', '26.03', 'See bundled License.txt', 'https://www.7-zip.org/',
        [(ROOT / 'tools/7zip/runtime/License.txt', 'tools/7zip/runtime/License.txt')])
    destination = ROOT / 'third-party'
    destination.mkdir(exist_ok=True)
    rust_version = subprocess.check_output(['rustc', '--version'], text=True).split()[1]
    sysroot = Path(subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip())
    rust_docs = sysroot / 'share/doc/rust'
    rust_notice = destination / 'rust' / rust_version
    rust_notice.mkdir(parents=True, exist_ok=True)
    rust_files = [rust_docs / 'COPYRIGHT-library.html', *sorted((rust_docs / 'licenses').glob('*.txt'))]
    assert rust_files[0].is_file() and len(rust_files) > 1, 'Install the rust-docs toolchain component for library notices.'
    copied = []
    for path in rust_files:
        target = rust_notice / path.relative_to(rust_docs)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
        copied.append((target, target.relative_to(destination).as_posix()))
    add('toolchain runtime', 'Rust standard library', rust_version,
        'See COPYRIGHT-library.html and referenced licenses', 'https://www.rust-lang.org/', copied)
    sources = json.loads((destination / 'sources/manifest.json').read_text(encoding='utf-8'))
    checksums = {(p['name'], p['version']): p.get('checksum') for p in
                 tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))['package']}
    for entry in entries:
        if entry['ecosystem'] == 'Cargo' and entry['declared_license'] == 'MPL-2.0':
            filename = f"{entry['name']}-{entry['version']}.crate"
            source = next((s for s in sources if s['file'] == filename), None)
            assert source and source['sha256'] == checksums[(entry['name'], entry['version'])], filename
    for source in sources:
        assert digest((destination / 'sources' / source['file']).read_bytes()) == source['sha256'], source['file']
    for supplemental in json.loads((destination / 'supplemental/manifest.json').read_text(encoding='utf-8')):
        path = destination / 'supplemental' / supplemental['file']
        assert digest(path.read_bytes()) == supplemental['sha256'], path
        sections.append(f'\nSupplemental notice: {supplemental["file"]}\n' + path.read_text(encoding='utf-8'))
    header = ('Galroon third-party notices\n\n'
              'Generated by scripts/generate_notices.py from Cargo.lock and package-lock.json.\n'
              'Windows Cargo graph excludes dev-only edges and includes build dependencies.\n'
              'npm inventory includes all lockfile packages not marked dev (a conservative superset).\n'
              'Nested source license files are included, even for optional vendored implementations.\n'
              'This is a source notice inventory, not a claim that every listed file is linked.\n'
              'Third-party works retain their own licenses. Galroon does not relicense them.\n'
              'Unmodified MPL crate sources and 7-Zip source are included in sources/.\n'
              'See README.md and sources/manifest.json for source locations and hashes.\n')
    (destination / 'THIRD_PARTY_NOTICES.txt').write_text(header + '\n'.join(sections) + '\n', encoding='utf-8')
    report = {'target': 'x86_64-pc-windows-msvc', 'cargo_lock_sha256': digest((ROOT / 'Cargo.lock').read_bytes()),
              'npm_lock_sha256': digest((ROOT / 'package-lock.json').read_bytes()),
              'packages': entries, 'source_archives': sources, 'missing_notice_text': missing}
    (destination / 'inventory.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'packages': len(entries), 'missing': missing,
                      'notice_bytes': (destination / 'THIRD_PARTY_NOTICES.txt').stat().st_size}))
    if missing:
        raise SystemExit('Notice text is missing; inventory is not ready to package.')


if __name__ == '__main__':
    main()
