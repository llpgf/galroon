"""Read-only checks of the files that a local Git repository would publish."""
from pathlib import Path
import json
import re
import subprocess
import sys
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]

def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args])

def main():
    files = sorted(set(p.decode('utf-8') for p in git('ls-files', '-z', '--cached', '--others', '--exclude-standard').split(b'\0') if p))
    problems = []
    local_links = []
    total = 0
    largest = []
    forbidden = re.compile(r'(^|/)(node_modules|target|dist|test-output|output|\.galroon|sandbox_data|local-history|\.playwright-cli)(/|$)|\.(?:sqlite3?|db)(?:-(?:wal|shm))?$|(^|/)(?:ui-session|device|active-library)\.json$|(^|/)\.env(?:\.(?!example$).*)?$|\.(?:pem|key|pfx|p12)$', re.I)
    secrets = re.compile(r'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|\bgh[pousr]_[A-Za-z0-9]{30,}\b|\bgithub_pat_[A-Za-z0-9_]{40,}\b|\bAKIA[0-9A-Z]{16}\b')
    text_ext = {'.md','.rs','.ts','.tsx','.js','.json','.toml','.yaml','.yml','.ps1','.py','.txt','.html','.css','.nsh'}
    for rel in files:
        p = ROOT / rel
        if not p.is_file():
            problems.append({'file':rel,'issue':'Missing candidate file'})
            continue
        if forbidden.search(rel):
            problems.append({'file':rel,'issue':'Local/private path is an upload candidate'})
        size = p.stat().st_size
        total += size
        largest.append((size,rel))
        if size > 50 * 1024 * 1024:
            problems.append({'file':rel,'issue':'File exceeds repository size budget (50 MiB)'})
        if p.suffix.lower() not in text_ext:
            continue
        content = p.read_text(encoding='utf-8-sig',errors='replace')
        if secrets.search(content):
            problems.append({'file':rel,'issue':'Possible credential material; inspect locally'})
        # Upstream license texts are preserved verbatim and may reference their original tree.
        if p.suffix != '.md' or rel.startswith(('third-party/upstream/', 'third-party/supplemental/', 'third-party/rust/')):
            continue
        for raw in re.findall(r'\]\(([^)\n]+)\)',content):
            target = raw.strip('<>')
            if re.match(r'^[a-zA-Z][\w+.-]*:',target) or target.startswith('#'):
                continue
            target = unquote(target.split('#',1)[0])
            full = (p.parent/target).resolve()
            try:
                linkrel = full.relative_to(ROOT).as_posix()
            except ValueError:
                problems.append({'file':rel,'issue':'Markdown link leaves repository','target':target})
                continue
            if forbidden.search(linkrel):
                local_links.append({'file':rel,'target':target})
            elif not full.exists():
                problems.append({'file':rel,'issue':'Missing Markdown link','target':target})
    config = json.loads((ROOT/'apps/desktop/src-tauri/tauri.conf.json').read_text(encoding='utf-8'))
    generated = {'../../../dist/','../../../target/release/galroon-core.exe'}
    for source in config['bundle']['resources']:
        if source not in generated and not (ROOT/'apps/desktop/src-tauri'/source).exists():
            problems.append({'file':'apps/desktop/src-tauri/tauri.conf.json','issue':'Missing source resource','target':source})
    probes = ['test-output/probe.txt','output/probe.txt','.env','.env.local','library.sqlite','library.sqlite-wal','ui-session.json','device.json','docs/local-history/probe.md','scripts/local-history/probe.ps1','target/probe.exe','node_modules/probe.js']
    for probe in probes:
        result = subprocess.run(['git','-C',str(ROOT),'check-ignore','-q','--no-index',probe])
        if result.returncode != 0:
            problems.append({'file':probe,'issue':'Expected ignore protection missing'})
    result = {'passed':not problems,'candidate_files':len(files),'candidate_bytes':total,'largest_files':[{'path':p,'bytes':n} for n,p in sorted(largest,reverse=True)[:8]],'local_evidence_links':local_links,'problems':problems,'scope':'Path, size, known credential patterns, local Markdown destinations and bundle source paths only; no app tests, staging or upload.'}
    print(json.dumps(result,ensure_ascii=False,indent=2))
    return 0 if not problems else 1

if __name__ == '__main__':
    sys.exit(main())
