"""Independent bounded v2 wire validation; no replacement for Rust semantic replay."""
import hashlib
import json
from pathlib import Path
import struct
import zlib
from capacity_report import LIMITS as V1_LIMITS, run_files, sha, read_json, process_entry, local, same_files

LIMITS = dict(max_decisions=256, max_manifest_bytes=16384, max_encoded_record_bytes=24*1024**2,
              max_physical_journal_bytes=64*1024**2, max_report_bytes=1024**2, max_snapshot_bytes=16*1024**2,
              max_decoded_record_bytes=24*1024**2, max_total_decoded_blob_bytes=256*1024**2)
ROOT = Path(__file__).resolve().parents[1]
D = b'MDC-REPLAY-RUN-002/'
h = lambda b: hashlib.sha256(b).digest()


def bounded(path, cap):
    with Path(path).open('rb') as f: b=f.read(cap+1)
    if len(b)>cap: raise ValueError('file bound')
    return b


def frame(data, kind, cap):
    magic=b'MDC-R'+dict(manifest=b'MAN',journal=b'JNL',decision=b'DEC',head=b'HED')[kind]+b'-002'+bytes(4)
    if len(data)<58 or data[:16]!=magic: raise ValueError('v2 frame magic/truncation')
    version,n=struct.unpack_from('<HQ',data,16)
    if version!=2 or n>cap or n+58!=len(data): raise ValueError('v2 frame length/version')
    digest=h(D+kind.encode()+b'/v2\0'+data[:-32])
    if digest!=data[-32:]: raise ValueError('v2 frame hash')
    return data[26:-32],digest


def blob(data, at, decoded_cap, encoded_cap, *, decode=True):
    if len(data)-at<49:raise ValueError('blob envelope')
    tag,n,e=struct.unpack_from('<BQQ',data,at)
    digest=data[at+17:at+49]; start=at+49; end=start+e
    if tag not in (0,1) or n>decoded_cap or e>encoded_cap or end>len(data) or (tag==0 and n!=e):raise ValueError('blob bounds/tag')
    if not decode:return n,end
    enc=data[start:end]
    if tag:
        if len(enc)<2 or enc[1]&32:raise ValueError('dictionary/header')
        d=zlib.decompressobj()
        try:
            raw=d.decompress(enc,n+1)
        except zlib.error as ex:raise ValueError('zlib stream') from ex
        if not d.eof or d.unconsumed_tail or d.unused_data or len(raw)!=n:raise ValueError('zlib EOF/output')
    else:raw=enc
    if h(raw)!=digest:raise ValueError('decoded hash')
    return raw,end


def manifest(path):
    encoded=bounded(path,16384); b,_=frame(encoded,'manifest',16384)
    if len(b)<130 or struct.unpack_from('<HHB',b)!=(1,1,1):raise ValueError('engine/consumer/identity')
    source_len,=struct.unpack_from('<Q',b,5); source_hash=b[13:45]
    symbol,unit,anchor,lag=struct.unpack_from('<BBBQ',b,45)
    if source_len>4*1024**2 or symbol>1 or unit>1 or anchor>1 or lag%1000 or (anchor==0 and lag<(1000 if unit==0 else 1)):raise ValueError('source/profile')
    values=struct.unpack_from('<8Q',b,56); limits=dict(zip(LIMITS,values))
    if any(not 0<v<=LIMITS[k] for k,v in limits.items()):raise ValueError('limit profile')
    n,=struct.unpack_from('<Q',b,122)
    if b[120:122]!=bytes([1,6]) or not 1<=n<=limits['max_decisions'] or len(b)!=130+8*n or len(encoded)>limits['max_manifest_bytes']:raise ValueError('codec/plan')
    plan=list(struct.unpack_from('<'+'Q'*n,b,130))
    if any(a>=b for a,b in zip(plan,plan[1:])):raise ValueError('plan order')
    return dict(body=b,encoded=encoded,run=h(D+b'run-id/v2\0'+b),source_len=source_len,source_hash=source_hash,plan=plan,limits=limits)


def decoded_bundles(folder):
    """Yields at most one bounded decoded record. Validates certified EOF at exhaustion."""
    folder=Path(folder); m=manifest(folder/'MANIFEST'); l=m['limits']; run=m['run']
    source=bounded(folder/'SOURCE',4*1024**2)
    if len(source)!=m['source_len'] or h(source)!=m['source_hash']:raise ValueError('source binding')
    head,_=frame(bounded(folder/'HEAD',512),'head',256)
    if len(head)!=144 or head[:32]!=run or head[32:64]!=h(m['encoded']):raise ValueError('HEAD binding')
    offset,count=struct.unpack_from('<QQ',head,64)
    if not 124<=offset<=l['max_physical_journal_bytes'] or count>len(m['plan']) or (folder/'JOURNAL').stat().st_size!=offset:raise ValueError('HEAD bound')
    total=0; latest=bytes(32)
    with (folder/'JOURNAL').open('rb') as stream:
        header=stream.read(124); b,previous=frame(header,'journal',128)
        if b!=struct.pack('<H',1)+run+h(m['encoded']):raise ValueError('header binding')
        for index in range(1,count+1):
            prefix=stream.read(26)
            if len(prefix)!=26:raise ValueError('missing frame')
            size,=struct.unpack_from('<Q',prefix,18)
            if size>l['max_encoded_record_bytes'] or stream.tell()+size+32>offset:raise ValueError('encoded frame bound')
            b,tip=frame(prefix+stream.read(size+32),'decision',l['max_encoded_record_bytes'])
            if len(b)<210 or b[:32]!=run or struct.unpack_from('<QQ',b,32)!=(index,m['plan'][index-1]) or b[80:112]!=previous:raise ValueError('decision binding')
            n,at=blob(b,112,l['max_report_bytes'],l['max_encoded_record_bytes'],decode=False)
            s,end=blob(b,at,l['max_snapshot_bytes'],l['max_encoded_record_bytes'],decode=False)
            if end!=len(b) or n+s+192>l['max_decoded_record_bytes'] or total+n+s>l['max_total_decoded_blob_bytes']:raise ValueError('decoded record/aggregate')
            report,at=blob(b,112,l['max_report_bytes'],l['max_encoded_record_bytes'])
            snapshot,end=blob(b,at,l['max_snapshot_bytes'],l['max_encoded_record_bytes'])
            total+=n+s
            info=json.loads(report)
            cutoff=m['plan'][index-1]
            if (not report.endswith(b'\n') or info['decision_us']!=cutoff or info['decision_id']!=b[48:80].hex()
                or info['raw_dataset_sha256']!=h(source).hex() or info['core_snapshot_sha256']!=h(snapshot).hex() or info['availability_quality']!='modeled'):
                raise ValueError('report context')
            key=b'BINANCE-SPOT-1M-MODELED-001/decision/v1\0'+m['body'][45:56]+struct.pack('<QQ',cutoff,len(info['eligible']))
            for e in info['eligible']:
                basis=bytes.fromhex(e['evidence_basis_hex'])
                if e['available_us']>cutoff or h(basis).hex()!=e['evidence_id']:raise ValueError('eligibility/basis')
                key+=struct.pack('<Q',len(basis))+basis
            if h(key)!=b[48:80]:raise ValueError('decision key')
            latest=h(D+b'decoded-bundle/v2\0'+b[:80]+h(report)+h(snapshot)); previous=tip
            yield dict(index=index,cutoff=cutoff,report=report,snapshot=snapshot,decision_id=b[48:80].hex())
        if stream.tell()!=offset or stream.read(1) or previous!=head[80:112] or latest!=head[112:]:raise ValueError('certified prefix EOF/tip')


def verify_run(folder, expected_source=None, profile=None):
    m=manifest(Path(folder)/'MANIFEST'); results=[]; total=0
    if expected_source is not None and bounded(Path(folder)/'SOURCE',4*1024**2)!=expected_source:raise ValueError('unlike captured source')
    if profile is not None:
        wanted=struct.pack('<BBBQ',0 if profile['symbol']=='BTCUSDT' else 1,0 if profile['unit']=='ms' else 1,0 if profile['anchor']=='close' else 1,profile['lag_ms']*1000)
        if m['body'][45:56]!=wanted:raise ValueError('unlike captured profile')
    for b in decoded_bundles(folder):
        results.append(dict(index=b['index'],cutoff=b['cutoff'],report_bytes=len(b['report']),snapshot_bytes=len(b['snapshot']),report_sha256=h(b['report']).hex(),snapshot_sha256=h(b['snapshot']).hex()))
        total+=len(b['report'])+len(b['snapshot'])
    return dict(run_id=m['run'].hex(),cutoffs=m['plan'],limits=m['limits'],committed=len(results),decoded_blob_bytes=total,bundles=results,
                file_sha256={n:sha(Path(folder)/n) for n in ('SOURCE','MANIFEST','HEAD','JOURNAL')},
                file_bytes={n:(Path(folder)/n).stat().st_size for n in ('SOURCE','MANIFEST','HEAD','JOURNAL')})


def v1_decoded(folder):
    # Called only after the existing independent v1 validator checked all bytes.
    with (Path(folder)/'JOURNAL').open('rb') as stream:
        stream.read(124)
        while prefix:=stream.read(26):
            size,=struct.unpack_from('<Q',prefix,18)
            if size>V1_LIMITS['max_record_bytes']:raise ValueError('v1 record cap')
            b=stream.read(size+32)[:-32]
            n,=struct.unpack_from('<Q',b,176); report=b[184:184+n]; snap=b[192+n:]
            yield report,snap


def compare_decoded(old,new,source,profile):
    a=run_files(old,source,profile); b=verify_run(new,source,profile)
    if a['cutoffs']!=b['cutoffs'] or a['run_id']==b['run_id'] or a['committed']>b['committed']:raise ValueError('unlike plan/audit/prefix')
    iterator=decoded_bundles(new)
    for report,snapshot in v1_decoded(old):
        other=next(iterator)
        if report!=other['report'] or snapshot!=other['snapshot']:raise ValueError('exact decoded bytes differ')
    for _ in iterator:pass
    return a,b


def validate_study(folder):
    from e2e_report import source_fingerprint
    from ci_metadata import ci_fingerprints
    folder=Path(folder); r=read_json(folder/'storage-study.json')
    if r['schema']!='MDC-STORAGE-STUDY-002/1' or r['status']!='COMPLETE' or type(r['repetitions'])is not int or not 3<=r['repetitions']<=10 or not r['points'] or r['points']!=sorted(set(r['points'])) or any(type(n)is not int or not 1<=n<=4096 for n in r['points']):raise ValueError('study matrix/status/repetitions')
    if (folder/'source.csv').stat().st_size>4*1024**2 or r['source_sha256']!=sha(folder/'source.csv') or r['source_bytes']!=(folder/'source.csv').stat().st_size:raise ValueError('study source')
    fp,files=source_fingerprint(ROOT)
    if r['build']['source_sha256']!=fp or r['build']['source_files']!=files or r['build']['ci_files']!=ci_fingerprints(ROOT):raise ValueError('study source/CI fingerprint')
    if r['build']['dirty'] and not r['build']['exploratory']:raise ValueError('dirty accepted build')
    process_entry(folder,r['build']['process'])
    if r['build']['process']['timeout'] or r['build']['process']['exit_code']:raise ValueError('unexecuted build')
    if r['build']['selected_flags']!={'CARGO_INCREMENTAL':'0','CARGO_PROFILE_RELEASE_DEBUG':'0','CARGO_PROFILE_RELEASE_OPT_LEVEL':'3'} or r['build']['profile']!='release':raise ValueError('build flags')
    if set(r['build']['binaries'])!={'v1','v2'} or r['profile']['availability_quality']!='modeled':raise ValueError('profile/binaries')
    if r['method']!={'clock':'parent perf_counter_ns / API Instant','reaping':'wait4','cpu_unit':'us','rss_unit':'KiB','cache':'uncontrolled/warm','timing_slo':'not_asserted','timeout_seconds':r['method']['timeout_seconds']} or not 1<=r['method']['timeout_seconds']<=600:raise ValueError('method/units')
    native={'x86_64':('x86_64',64,'little'),'aarch64':('aarch64',64,'little'),'i686':('x86',32,'little'),'s390x':('s390x',64,'big')}.get(r['host']['machine'])
    if native is None or r['build']['target']!=dict(zip(('arch','pointer_width','endian'),native)):raise ValueError('compiled native target')
    for name,entry in r['build']['binaries'].items():
        if sha(local(folder,entry['file']))!=entry['sha256']:raise ValueError('binary digest')
    wanted=[(n,j) for n in r['points'] for j in range(1,r['repetitions']+1)]
    if [(c['requested'],c['repetition']) for c in r['cases']]!=wanted:raise ValueError('missing/duplicate study case')
    references={}
    for c in r['cases']:
        for version in ('v1','v2'):
            row=c[version]
            prefix=f"cases/n{c['requested']:04}-r{c['repetition']:02}-{version}"
            if row['directory']!=prefix+'/run':raise ValueError('reused sample')
            for phase,entry in row['phases'].items():
                if entry['stdout_file']!=prefix+'/'+phase+'.stdout' or entry['stderr_file']!=prefix+'/'+phase+'.stderr':raise ValueError('reused phase log')
                process_entry(folder,entry)
                if entry['timeout'] or entry['exit_code']:raise ValueError('phase did not finish')
                value=read_json(local(folder,entry['stdout_file']))
                if value['target']!=r['build']['target']:raise ValueError('mixed native target')
                if value['phase']!=phase or value['schema']!=('MDC-CAPACITY-PHASE-001' if version=='v1' else 'MDC-STORAGE-V2-PHASE-001'):raise ValueError('phase identity')
                for k,n in value.items():
                    if k.endswith('_ns') and (type(n)is not int or n<0 or n>entry['wall_ns']+10000000):raise ValueError('phase clock')
            create=read_json(local(folder,row['phases']['create']['stdout_file']))
            expected_limits=V1_LIMITS if version=='v1' else LIMITS
            if create['requested']!=c['requested'] or create['source_sha256']!=r['source_sha256'] or create['limits']!=expected_limits:raise ValueError('wrong requested plan/source/limits')
            if create['replay_limits']!={'max_input_bytes':4194304,'max_rows':4096,'max_line_bytes':4096,'max_visible_bars':256}:raise ValueError('replay limits')
            if row['outcome']=='PreflightRejected':
                if create['outcome']!='PreflightRejected' or c['requested']<=256 or set(row['phases'])!={'create'} or local(folder,row['directory']).exists():raise ValueError('preflight output')
                continue
            if create['outcome']!='Created':raise ValueError('create outcome')
            commit=read_json(local(folder,row['phases']['commit']['stdout_file'])); recover=read_json(local(folder,row['phases']['recover']['stdout_file']))
            if commit['outcome'] not in ('Completed','Limited') or recover['outcome']!='Recovered' or commit['outcome']!=row['outcome']:raise ValueError('commit/recovery outcome')
            actual=run_files(local(folder,row['directory']),(folder/'source.csv').read_bytes(),r['profile']) if version=='v1' else verify_run(local(folder,row['directory']),(folder/'source.csv').read_bytes(),r['profile'])
            if row['validation']!=actual or commit['bundles']!=actual['bundles'] or recover['bundles']!=actual['bundles'] or commit['committed']!=actual['committed'] or recover['committed']!=actual['committed']:raise ValueError('retained bundle/cursor diagnostics')
            if create['limits']!=expected_limits or recover['limits']!=expected_limits:raise ValueError('default limits changed')
            if commit['outcome']=='Limited' and (commit.get('failed_head_unchanged')is not True or commit.get('failed_journal_length_unchanged')is not True):raise ValueError('failed limit wrote prefix')
            if version=='v2' and (recover['decoded_blob_bytes']!=actual['decoded_blob_bytes'] or recover['physical_journal_bytes']!=actual['file_bytes']['JOURNAL']):raise ValueError('physical/decoded diagnostics')
            count=actual['committed']; next_index=None if count==c['requested'] else count+1
            if create['committed']!=0 or create['cutoffs']!=actual['cutoffs'] or commit['cutoffs']!=actual['cutoffs'] or recover['cutoffs']!=actual['cutoffs'] or commit['next_index']!=next_index or recover['next_index']!=next_index or recover['run_id']!=actual['run_id']:raise ValueError('cursor/plan/run')
            steps=commit['steps']
            if [v['index'] for v in steps]!=list(range(1,len(steps)+1)) or any(type(v['api_ns'])is not int or v['api_ns']<0 for v in steps) or sum(v['api_ns'] for v in steps)>row['phases']['commit']['wall_ns']:raise ValueError('step sequence/clock')
            if commit['outcome']=='Completed' and (count!=c['requested'] or len(steps)!=count or any(v['outcome']!='Committed' for v in steps)):raise ValueError('false completion')
            if commit['outcome']=='Limited' and (count>=c['requested'] or commit['error_class']!='Limit' or commit['failed_index']!=count+1 or len(steps)!=count+1 or steps[-1]['outcome']!='Limited' or any(v['outcome']!='Committed' for v in steps[:-1])):raise ValueError('false limit')
            if version=='v2' and (commit['decoded_blob_bytes']!=actual['decoded_blob_bytes'] or commit['physical_journal_bytes']!=actual['file_bytes']['JOURNAL']):raise ValueError('commit physical/decoded')
            if set(row['phases'])!=({'create','commit','recover','retry'} if count else {'create','commit','recover'}):raise ValueError('missing/unexpected phase')
            reference=references.setdefault((version,c['requested']),local(folder,row['directory']))
            same_files(reference,local(folder,row['directory']))
            if actual['committed']:
                retry=read_json(local(folder,row['phases']['retry']['stdout_file']))
                if retry.get('retry_head_unchanged')is not True or retry.get('retry_journal_length_unchanged')is not True or retry['retry_index']!=1 or retry['committed']!=count or retry['bundles']!=actual['bundles']:raise ValueError('owned retry changed prefix')
        if c['v1']['outcome']=='PreflightRejected' or c['v2']['outcome']=='PreflightRejected':
            if c['v1']['outcome']!=c['v2']['outcome']:raise ValueError('preflight differs')
        else:
            compare_decoded(local(folder,c['v1']['directory']),local(folder,c['v2']['directory']),(folder/'source.csv').read_bytes(),r['profile'])
    return dict(acceptance='PASS',source_sha256=fp,cases=len(r['cases']),repetitions=r['repetitions'],exact_common_prefix_bytes=True,physical_identity='versioned_distinct',timing_slo='not_asserted')
