"""Pinned 016 vs current exact replay comparison; never rebind old evidence."""
import argparse
import contextlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile
from capacity_report import read_json, sha, validate, same_files, run_files
from ci_metadata import ci_fingerprints
from e2e_report import source_fingerprint

ROOT = Path(__file__).resolve().parents[1]
BASE = '6b33a4375384ed4301695ea879e641a20f5493b9'
BASE_FP = 'cca5b3a70dea13130f8fc25f91152b5fd4dfecf71fabd731eb1cd0f11ae20de4'


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()


def candidate_target():
    # Cargo resolves a relative target directory from the source checkout.
    return (ROOT / os.environ.get('CARGO_TARGET_DIR','target')).resolve()


def baseline_target():
    # Stay inside the ignored target tree; never create untracked target-baseline.
    return candidate_target() / 'exact-work-baseline'


@contextlib.contextmanager
def baseline():
    try:
        git('cat-file', '-e', BASE+'^{commit}')
    except subprocess.CalledProcessError:
        subprocess.run(['git', 'fetch', '--no-tags', '--depth=1', 'origin', BASE], cwd=ROOT, check=True)
    with tempfile.TemporaryDirectory(prefix='mdc-exact-work-') as temp:
        path = Path(temp)/'source'
        subprocess.run(['git', 'worktree', 'add', '--detach', '--no-checkout', str(path), BASE], cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
        try:
            subprocess.run(['git', '-C', str(path), 'sparse-checkout', 'set', '--no-cone', '/Cargo.toml', '/Cargo.lock', '/.gitignore', '/src/', '/examples/', '/tests/', '/scripts/', '/adapters/', '/.github/', '/docs/evidence/wal-framing-v1.json'], check=True)
            subprocess.run(['git', '-C', str(path), 'checkout', '--detach', BASE], check=True, stdout=subprocess.DEVNULL)
            fp, entries = source_fingerprint(path)
            expected = json.loads(git('show', BASE+':docs/evidence/impl-016-source-manifest.json'))
            if fp != BASE_FP or entries != expected['source_files'] or subprocess.check_output(['git','status','--porcelain=v1','-uall'],cwd=path).strip():
                raise ValueError('baseline source is not the pinned clean revision')
            yield path
        finally:
            subprocess.run(['git', 'worktree', 'remove', '--force', str(path)], cwd=ROOT, check=True)


def validate_old(base, folder):
    # Execute the frozen validator only after its entire source map was checked.
    code = "import sys,json;sys.path.insert(0,sys.argv[1]);from capacity_report import validate;print(json.dumps(validate(sys.argv[2])))"
    return json.loads(subprocess.check_output([sys.executable, '-I', '-B', '-c', code, str(base/'scripts'), str(folder.resolve())], cwd=base, text=True))


def compare_folders(base, a, b):
    checked_a, checked_b = validate_old(base, a), validate(b)
    left, right = read_json(a/'capacity-report.json'), read_json(b/'capacity-report.json')
    if left['build']['commit_sha'] != BASE or left['build']['worktree_dirty'] is not False or left['build']['implementation_source_sha256'] != BASE_FP:
        raise ValueError('baseline build is not pinned')
    if left['build']['tree_sha'] != git('rev-parse', BASE+'^{tree}') or left['build']['ci_file_sha256'] != ci_fingerprints(base):
        raise ValueError('baseline tree/CI differs')
    for key in ('points','repetitions','profile','limits','probe_kind'):
        if left[key] != right[key]: raise ValueError('unlike study configurations')
    for key in ('sha256','bytes','origin'):
        if left['source'][key] != right['source'][key]: raise ValueError('unlike input')
    if (a/'source.csv').read_bytes() != (b/'source.csv').read_bytes(): raise ValueError('source bytes differ')
    if left['build']['rustc'] != right['build']['rustc'] or left['build']['target'] != right['build']['target'] or left['host'] != right['host']:
        raise ValueError('unlike compiler/target/host')
    for x,y in zip(left['cases'],right['cases']):
        if tuple(x.get(k) for k in ('requested','repetition','outcome','committed','first_constraint')) != tuple(y.get(k) for k in ('requested','repetition','outcome','committed','first_constraint')):
            raise ValueError('outcome/cursor/constraint changed')
        if 'file_sha256' in x: same_files(a/x['directory'],b/y['directory'])
    current_fp,current_files=source_fingerprint(ROOT)
    # Core/store/codec/defaults/old tests remain exact; permitted runtime files are explicit.
    allowed={'adapters/mdc_replay/src/'+n for n in ('lib.rs','core_view.rs','recovery.rs','recovery/store.rs','recovery/store/tests.rs','work_counts.rs')}
    old_files=left['build']['source_files']
    for path, digest in old_files.items():
        if path in allowed or path.startswith('scripts/') or path=='adapters/mdc_replay/README.md': continue
        if current_files.get(path)!=digest:raise ValueError('protected source changed: '+path)
    return dict(acceptance='PASS', baseline_commit=BASE,baseline_source_sha256=BASE_FP,candidate_source_sha256=current_fp,
                baseline_build=left['build'],candidate_build=right['build'],baseline_checked=checked_a,candidate_checked=checked_b,
                source=right['source'],profile=right['profile'],points=right['points'],repetitions=right['repetitions'],limits=right['limits'],host=right['host'],
                baseline_summary=left['summary'],candidate_summary=right['summary'],
                cases=[dict(requested=x['requested'],repetition=x['repetition'],outcome=x['outcome'],committed=x['committed'],directory=x['directory'],first_constraint=x.get('first_constraint')) for x in right['cases']],
                full_run_files_equal=True,timing_slo='not_asserted')


def compatibility(base, b, out):
    out.mkdir()
    report=read_json(b/'capacity-report.json')
    cell=next((c for c in report['cases'] if c['requested']>=3 and c['outcome']!='PreflightRejected'),None)
    if cell is None:raise ValueError('compatibility proof needs at least three normalized cutoffs')
    create=read_json(b/cell['phases']['create']['stdout_file'])
    plan=','.join(map(str,create['cutoffs'][:3]));profile=report['profile'];commands=[]
    for name,source in [('old',base),('new',ROOT)]:
        env=os.environ.copy();env.update(CARGO_INCREMENTAL='0',CARGO_PROFILE_RELEASE_DEBUG='0',CARGO_PROFILE_RELEASE_OPT_LEVEL='3')
        target=str(baseline_target() if name=='old' else candidate_target())
        env['CARGO_TARGET_DIR']=target
        cmd=['cargo','build','--locked','--offline','--release','--manifest-path',str(source/'adapters/mdc_replay/Cargo.toml'),'--bin','mdc-replay-run']
        r=subprocess.run(cmd,cwd=source,env=env,capture_output=True)
        (out/(name+'-build.stdout')).write_bytes(r.stdout);(out/(name+'-build.stderr')).write_bytes(r.stderr)
        r.check_returncode();shutil.copy2(Path(target)/'release/mdc-replay-run',out/(name+'-cli'))
    def call(label,variant,*args):
        r=subprocess.run([str(out/(variant+'-cli')),*map(str,args)],capture_output=True,timeout=120)
        (out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr)
        commands.append(dict(label=label,variant=variant,args=list(map(str,args)),exit=r.returncode,stdout_sha256=sha(out/(label+'.stdout')),stderr_sha256=sha(out/(label+'.stderr'))))
        r.check_returncode();return r.stdout
    tail=[b/'source.csv',profile['symbol'],profile['unit'],str(profile['lag_ms']),profile['anchor'],plan]
    call('create-old','old','create',out/'continued',*tail)
    call('step-old','old','step',out/'continued','1')
    shutil.copytree(out/'continued',out/'old-prefix')
    before={n:sha(out/'continued'/n) for n in ['SOURCE','MANIFEST','HEAD','JOURNAL']}
    call('retry-old','new','step',out/'continued','1')
    if before!={n:sha(out/'continued'/n) for n in before}:raise ValueError('old-prefix retry changed bytes')
    call('step-candidate','new','step',out/'continued','2')
    call('finish-candidate','new','finish',out/'continued')
    call('create-reference','new','create',out/'reference',*tail)
    call('finish-reference','new','finish',out/'reference')
    if b'durable_through=1' not in call('recover-old-prefix','new','status',out/'old-prefix'):raise ValueError('old prefix not recovered')
    same_files(out/'continued',out/'reference')
    record=dict(profile=profile,commands=commands,binaries={name:sha(out/(name+'-cli')) for name in ['old','new']})
    (out/'compatibility.json').write_text(json.dumps(record,indent=2)+'\n')
    check_compatibility(out,b)


def check_compatibility(folder,candidate):
    record=read_json(folder/'compatibility.json');report=read_json(candidate/'capacity-report.json')
    if record['profile']!=report['profile']:raise ValueError('compatibility profile differs')
    for name,digest in record['binaries'].items():
        if name not in ['old','new'] or sha(folder/(name+'-cli'))!=digest:raise ValueError('compatibility executable changed')
    if set(record['binaries'])!={'old','new'}:raise ValueError('compatibility binaries absent')
    labels=['create-old','step-old','retry-old','step-candidate','finish-candidate','create-reference','finish-reference','recover-old-prefix']
    if [c['label'] for c in record['commands']]!=labels:raise ValueError('compatibility steps missing')
    if [c['variant'] for c in record['commands']]!=['old','old']+['new']*6:raise ValueError('wrong compatibility writer sequence')
    for c in record['commands']:
        if c['exit'] or sha(folder/(c['label']+'.stdout'))!=c['stdout_sha256'] or sha(folder/(c['label']+'.stderr'))!=c['stderr_sha256']:raise ValueError('compatibility execution differs')
    source=(candidate/'source.csv').read_bytes()
    for name,count in [('old-prefix',1),('continued',3),('reference',3)]:
        actual=run_files(folder/name,source,report['profile'])
        if actual['committed']!=count:raise ValueError('compatibility cursor differs')
    same_files(folder/'continued',folder/'reference')
    prefix=(folder/'old-prefix/JOURNAL').read_bytes()
    if not (folder/'continued/JOURNAL').read_bytes().startswith(prefix):raise ValueError('continued journal replaced the old prefix')


def archive(folder, path):
    with zipfile.ZipFile(path,'x',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as z:
        for p in sorted(folder.rglob('*')):
            if p.is_file():z.write(p,str(p.relative_to(folder)))


def unpack(path, target):
    with zipfile.ZipFile(path) as z:
        names=z.namelist()
        if len(names)!=len(set(names)) or sum(x.file_size for x in z.infolist())>2*1024**3:raise ValueError('archive bounds/duplicates')
        for x in z.infolist():
            p=Path(x.filename)
            if p.is_absolute() or '..' in p.parts or (x.external_attr>>16)&0o170000==0o120000:raise ValueError('unsafe archive member')
        z.extractall(target)


def check_pair(folder, base):
    receipt=read_json(folder/'pair.json')
    if receipt['schema']!='MDC-EXACT-WORK-001/1' or receipt['comparison']['baseline_commit']!=BASE:raise ValueError('unapproved baseline/schema')
    if set(receipt['archives'])!={'baseline','candidate','compatibility'} or receipt['order'] not in (['baseline','candidate'],['candidate','baseline']):raise ValueError('archive/order set')
    for name, info in receipt['archives'].items():
        if info['file']!=name+'.zip' or sha(folder/info['file'])!=info['sha256']:raise ValueError('archive digest')
    with tempfile.TemporaryDirectory(prefix='mdc-exact-check-') as temp:
        a,b=Path(temp)/'baseline',Path(temp)/'candidate'
        unpack(folder/'baseline.zip',a);unpack(folder/'candidate.zip',b)
        compat=Path(temp)/'compatibility';unpack(folder/'compatibility.zip',compat);check_compatibility(compat,b)
        actual=compare_folders(base,a,b)
        if actual!=receipt['comparison']:raise ValueError('stale or forged comparison receipt')
    return receipt


def run(args):
    if args.output.exists():raise ValueError('output already exists')
    args.output.mkdir(parents=True)
    common=['--points',args.points,'--repetitions',str(args.repetitions),'--unit',args.unit,'--source-label',args.source_label,'--journal-budget-bytes',str(args.journal_budget_bytes)]
    if args.synthetic_day:common+=['--synthetic-day']
    else:
        if not args.expected_source_sha256:raise ValueError('provided source needs pinned digest')
        common+=['--source',str(args.source.resolve()),'--expected-source-sha256',args.expected_source_sha256]
    order=['baseline','candidate'] if os.environ.get('RUSTUP_TOOLCHAIN','').startswith('1.85.1') else ['candidate','baseline']
    with baseline() as old, tempfile.TemporaryDirectory(prefix='mdc-exact-study-') as temp:
        folders={n:Path(temp)/n for n in ['baseline','candidate','compatibility']}
        try:
            for name in order:
                source=old if name=='baseline' else ROOT
                env=os.environ.copy()
                if name=='baseline':env['CARGO_TARGET_DIR']=str(baseline_target())
                subprocess.run([sys.executable,'-B',str(source/'scripts/run_capacity.py'),*common,'--output',str(folders[name])],cwd=source,env=env,check=True)
            comparison=compare_folders(old,folders['baseline'],folders['candidate'])
            compatibility(old,folders['candidate'],folders['compatibility'])
            archives={}
            for name in folders:
                archive(folders[name],args.output/(name+'.zip'))
                archives[name]=dict(file=name+'.zip',sha256=sha(args.output/(name+'.zip')))
            receipt=dict(schema='MDC-EXACT-WORK-001/1',order=order,cache_state='uncontrolled; variants grouped, reversed across toolchains, not randomized',archives=archives,comparison=comparison)
            (args.output/'pair.json').write_text(json.dumps(receipt,indent=2)+'\n')
            check_pair(args.output,old)
        except BaseException as error:
            for name in folders:
                if folders[name].exists() and not (args.output/(name+'.zip')).exists():
                    archive(folders[name],args.output/(name+'.zip'))
            (args.output/'pair-error.json').write_text(json.dumps(dict(status='INCOMPLETE',error=str(error),baseline=BASE,order=order),indent=2)+'\n')
            raise
    print(json.dumps(dict(acceptance='PASS',cases=len(comparison['cases']),variants=2,order=order)))


def compare_ci(root, study):
    rows=[];seen=set();expected=git('rev-parse','HEAD');run_id=os.environ.get('GITHUB_RUN_ID')
    with baseline() as old:
        for p in sorted(root.rglob('pair.json')):
            r=check_pair(p.parent,old);c=r['comparison'];v=c['candidate_build']['rustc'].split()[1]
            metadata_path=next((x/'metadata.json' for x in p.parents if (x/'metadata.json').exists() and (x==root or root in x.parents)),None)
            if metadata_path is None:raise ValueError('missing CI metadata')
            m=read_json(metadata_path)
            if (v in seen or c['candidate_build']['commit_sha']!=expected or c['candidate_build']['worktree_dirty'] is not False or m['commit_sha']!=expected or m['github_run_id']!=run_id
                or not run_id or m['status']!='PASS' or m['worktree_dirty'] is not False or m['ci_file_sha256']!=ci_fingerprints(ROOT) or m['source_files']!=source_fingerprint(ROOT)[1]
                or m['implementation_source_sha256']!=source_fingerprint(ROOT)[0] or c['candidate_build']['rustc']!=m['rustc']
                or m['profile']!=('capacity-study' if study else 'binance-replay')):raise ValueError('mixed/stale/dirty CI pair')
            if not study and (c['points']!=[1,2,3,1440] or c['repetitions']!=3 or c['source']['origin']!='synthetic_fixture' or c['limits']['max_journal_bytes']!=32768):raise ValueError('regular paired smoke changed')
            seen.add(v);rows.append(dict(rust=v,receipt=r,artifact=str(p.parent.relative_to(root))))
    if seen!={'1.85.1','1.98.0'}:raise ValueError('incomplete toolchain pair matrix')
    first,second=(r['receipt']['comparison'] for r in rows)
    for key in ['points','repetitions','profile','limits','cases']:
        if first[key]!=second[key]:raise ValueError('unlike cross-toolchain configurations')
    if first['source']['sha256']!=second['source']['sha256']:raise ValueError('unlike cross-toolchain inputs')
    for variant in ['baseline','candidate']:
        with zipfile.ZipFile(root/rows[0]['artifact']/(variant+'.zip')) as a,zipfile.ZipFile(root/rows[1]['artifact']/(variant+'.zip')) as b:
            for cell in rows[0]['receipt']['comparison']['cases']:
                if cell['outcome']=='PreflightRejected':continue
                for name in ['SOURCE','MANIFEST','HEAD','JOURNAL']:
                    path=cell['directory']+'/'+name
                    with a.open(path) as x,b.open(path) as y:
                        while True:
                            left,right=x.read(1048576),y.read(1048576)
                            if left!=right:raise ValueError('cross-toolchain retained bytes differ')
                            if not left:break
    return dict(acceptance='PASS',commit=expected,github_run_id=run_id,configurations=rows,timing_slo='not_asserted')


def main():
    p=argparse.ArgumentParser();sub=p.add_subparsers(dest='mode',required=True)
    r=sub.add_parser('run');s=r.add_mutually_exclusive_group(required=True);s.add_argument('--source',type=Path);s.add_argument('--synthetic-day',action='store_true')
    r.add_argument('--expected-source-sha256');r.add_argument('--source-label',default='exact-work-study');r.add_argument('--unit',choices=['ms','us'],default='ms')
    r.add_argument('--points',default='1,2,3,1440');r.add_argument('--repetitions',type=int,default=3);r.add_argument('--journal-budget-bytes',type=int,default=67108864);r.add_argument('--output',type=Path,required=True)
    c=sub.add_parser('check');c.add_argument('--folder',type=Path,required=True)
    c=sub.add_parser('compare-ci');c.add_argument('--root',type=Path,required=True);c.add_argument('--study',action='store_true');c.add_argument('--output',type=Path,required=True)
    a=p.parse_args()
    if a.mode=='run':run(a)
    elif a.mode=='check':
        with baseline() as old:r=check_pair(a.folder,old)
        print(json.dumps(dict(acceptance='PASS',cases=len(r['comparison']['cases']))))
    else:
        result=compare_ci(a.root.resolve(),a.study);a.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(dict(acceptance='PASS',configurations=len(result['configurations']))))


if __name__=='__main__':main()
