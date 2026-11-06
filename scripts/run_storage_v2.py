"""Opt-in paired v1/v2 release study, preserving every raw sample and failure."""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
from capacity_report import read_json, sha, run_files, process_entry, same_files
from run_capacity import invoke, parsed, synthetic, output_bytes, cpu_model
from e2e_report import source_fingerprint
from ci_metadata import ci_fingerprints
from storage_v2 import ROOT, verify_run, validate_study, compare_decoded


def run(a):
    points=[int(n) for n in a.points.split(',')]
    if not points or points!=sorted(set(points)) or len(points)>8 or any(not 1<=n<=4096 for n in points) or not 3<=a.repetitions<=10 or not 1<=a.timeout<=600:
        raise ValueError('invalid points/repetitions/timeout')
    if platform.system()!='Linux' or any(os.environ.get(k) for k in ('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS')):raise ValueError('controlled Linux release profile required')
    if a.synthetic_day:source=synthetic(a.unit)
    else:
        with a.source.open('rb') as stream:source=stream.read(4*1024**2+1)
    if len(source)>4*1024**2 or (not a.synthetic_day and (not a.expected_source_sha256 or __import__('hashlib').sha256(source).hexdigest()!=a.expected_source_sha256)):
        raise ValueError('source size/hash')
    out=a.output.resolve()
    for name in ('src','examples','tests','scripts','adapters'):
        if out==ROOT/name or ROOT/name in out.parents:raise ValueError('output is inside sources')
    out.mkdir(parents=True,exist_ok=False); (out/'source.csv').write_bytes(source)
    fp,files=source_fingerprint(ROOT)
    git=lambda *args:subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
    dirty=bool(git('status','--porcelain=v1','-uall'))
    if dirty and not a.allow_dirty:raise ValueError('clean build required; --allow-dirty only for exploratory local measurements')
    env=os.environ.copy();env.update(CARGO_INCREMENTAL='0',CARGO_PROFILE_RELEASE_DEBUG='0',CARGO_PROFILE_RELEASE_OPT_LEVEL='3')
    names={'v1':'mdc-replay-capacity','v2':'mdc-replay-v2-capacity'}
    command=['cargo','build','--locked','--offline','--release','--manifest-path',str(ROOT/'adapters/mdc_replay/Cargo.toml')]
    for name in names.values():command+=['--bin',name]
    build=invoke(command,out,'build',600,env=env)
    if build['exit_code'] or build['timeout']:raise ValueError('release build failed')
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--offline','--no-deps','--format-version','1','--manifest-path',str(ROOT/'adapters/mdc_replay/Cargo.toml')],cwd=ROOT,env=env))
    (out/'bin').mkdir(); binaries={}
    for kind,name in names.items():
        target=out/'bin'/name;shutil.copy2(Path(metadata['target_directory'])/'release'/name,target)
        binaries[kind]=dict(file='bin/'+name,sha256=sha(target))
    profile=dict(symbol='BTCUSDT',unit=a.unit,lag_ms=100,anchor='close',availability_quality='modeled')
    report=dict(schema='MDC-STORAGE-STUDY-002/1',status='RUNNING',points=points,repetitions=a.repetitions,profile=profile,
                source_sha256=sha(out/'source.csv'),source_bytes=len(source),source_origin='synthetic' if a.synthetic_day else 'provided_hash_checked_csv',
                method=dict(clock='parent perf_counter_ns / API Instant',reaping='wait4',cpu_unit='us',rss_unit='KiB',cache='uncontrolled/warm',timing_slo='not_asserted',timeout_seconds=a.timeout),
                host=dict(os=platform.platform(),cpu=cpu_model(),cpus=os.cpu_count(),machine=platform.machine()),
                build=dict(commit=git('rev-parse','HEAD'),tree=git('rev-parse','HEAD^{tree}'),dirty=dirty,exploratory=a.allow_dirty,source_sha256=fp,source_files=files,ci_files=ci_fingerprints(ROOT),
                           rustc=subprocess.check_output(['rustc','-vV'],env=env,text=True).strip(),cargo=subprocess.check_output(['cargo','--version'],env=env,text=True).strip(),
                           binaries=binaries,process=build,command=command,profile='release',selected_flags={k:env[k] for k in ('CARGO_INCREMENTAL','CARGO_PROFILE_RELEASE_DEBUG','CARGO_PROFILE_RELEASE_OPT_LEVEL')}),cases=[])
    def save(): (out/'storage-study.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    for n in points:
        for rep in range(1,a.repetitions+1):
            case=dict(requested=n,repetition=rep)
            for kind in ('v1','v2'):
                key=f'cases/n{n:04}-r{rep:02}-{kind}'; directory=out/key/'run';directory.parent.mkdir(parents=True)
                if output_bytes(out)+72*1024**2>2048*1024**2 or shutil.disk_usage(out).free<140*1024**2:raise ValueError('study output/free-space budget; no samples deleted')
                row=dict(directory=str(directory.relative_to(out)),phases={});case[kind]=row
                driver=out/binaries[kind]['file']
                def call(phase,*tail):
                    entry=invoke([driver,phase,directory,*tail],out,key+'/'+phase,a.timeout,env=env)
                    row['phases'][phase]=entry;value=parsed(out,entry)
                    if value is None:raise ValueError('phase timeout/failure; outputs retained')
                    return value
                create=call('create',out/'source.csv','BTCUSDT',a.unit,'100','close',n)
                report['build'].setdefault('target',create['target'])
                if create['outcome']=='PreflightRejected':row['outcome']='PreflightRejected';continue
                if create['outcome']!='Created':raise ValueError('create failed')
                commit=call('commit');recover=call('recover')
                if commit['outcome'] not in ('Completed','Limited') or recover['outcome']!='Recovered':raise ValueError('commit/recovery failed')
                row['outcome']=commit['outcome'];row['validation']=run_files(directory,source,profile) if kind=='v1' else verify_run(directory,source,profile)
                if row['validation']['committed']:call('retry',1) # oldest view, owned read after restart
            if case['v1']['outcome']!='PreflightRejected' and case['v2']['outcome']!='PreflightRejected':
                compare_decoded(out/case['v1']['directory'],out/case['v2']['directory'],source,profile)
            report['cases'].append(case);save()
            print(json.dumps(dict(requested=n,repetition=rep,v1=case['v1']['outcome'],v2=case['v2']['outcome'])),flush=True)
    if source_fingerprint(ROOT)[0]!=fp or ci_fingerprints(ROOT)!=report['build']['ci_files']:raise ValueError('source/CI changed during study')
    report['status']='COMPLETE';save()
    result=validate_study(out);(out/'storage-validation.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)


def compare(root):
    folders=sorted(Path(root).rglob('storage-study.json'))
    if len(folders)!=2:raise ValueError('exactly two native studies required')
    checked=[validate_study(p.parent) for p in folders]; reports=[read_json(p) for p in folders]
    a,b=reports
    for key in ('points','repetitions','profile','source_sha256','source_bytes'):
        if a[key]!=b[key]:raise ValueError('unlike study '+key)
    if {r['build']['rustc'].splitlines()[0].split()[1] for r in reports}!={'1.85.1','1.98.0'}:raise ValueError('missing toolchain')
    for ca,cb in zip(a['cases'],b['cases']):
        for kind in ('v1','v2'):
            if ca[kind]['outcome']!=cb[kind]['outcome'] or ca[kind].get('validation')!=cb[kind].get('validation'):raise ValueError('cross-toolchain outcomes/bytes differ')
            if 'validation' in ca[kind]:same_files(folders[0].parent/ca[kind]['directory'],folders[1].parent/cb[kind]['directory'])
    return dict(acceptance='PASS',studies=checked,cases_per_study=len(a['cases']),full_files_equal=True,points=a['points'],source_sha256=a['source_sha256'])


def main():
    p=argparse.ArgumentParser();sub=p.add_subparsers(dest='cmd',required=True)
    r=sub.add_parser('run');source=r.add_mutually_exclusive_group(required=True);source.add_argument('--source',type=Path);source.add_argument('--synthetic-day',action='store_true')
    r.add_argument('--expected-source-sha256');r.add_argument('--unit',choices=['ms','us'],default='ms');r.add_argument('--points',default='1,3,1440');r.add_argument('--repetitions',type=int,default=3);r.add_argument('--timeout',type=float,default=600);r.add_argument('--allow-dirty',action='store_true');r.add_argument('--output',required=True,type=Path)
    c=sub.add_parser('check');c.add_argument('--folder',type=Path,required=True)
    c=sub.add_parser('compare');c.add_argument('--root',type=Path,required=True);c.add_argument('--output',type=Path)
    a=p.parse_args()
    if a.cmd=='run':run(a)
    else:
        result=validate_study(a.folder) if a.cmd=='check' else compare(a.root)
        if getattr(a,'output',None):a.output.write_text(json.dumps(result,indent=2)+'\n')
        print(json.dumps(result),flush=True)

if __name__=='__main__':main()
