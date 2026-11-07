import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from storage_v2 import blob, frame, h, validate_study, verify_run, ROOT

class StorageValidatorTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.folder=Path(self.temp.name)/'study'
        shutil.copytree(Path(os.environ['MDC_STORAGE_V2_DIR']),self.folder)
        self.path=self.folder/'storage-study.json';self.report=json.loads(self.path.read_text())
    def save(self):self.path.write_text(json.dumps(self.report))
    def reject(self):
        self.save()
        with self.assertRaises((ValueError,FileNotFoundError,KeyError)):validate_study(self.folder)
    def phase(self,name,edit):
        e=self.report['cases'][0]['v2']['phases'][name];p=self.folder/e['stdout_file'];v=json.loads(p.read_text());edit(v);p.write_text(json.dumps(v));e['stdout_sha256']=h(p.read_bytes()).hex()
    def test_real_study_passes(self):self.assertEqual(validate_study(self.folder)['acceptance'],'PASS')
    def test_missing_case(self):self.report['cases'].pop();self.reject()
    def test_duplicate_case(self):self.report['cases'][1]=copy.deepcopy(self.report['cases'][0]);self.reject()
    def test_incomplete_build(self):self.report['build']['process']['exit_code']=1;self.reject()
    def test_stale_source(self):self.report['build']['source_sha256']='0'*64;self.reject()
    def test_unexecuted_recovery(self):self.report['cases'][0]['v2']['phases']['recover']['timeout']=True;self.reject()
    def test_log_replacement(self):e=self.report['cases'][0]['v2']['phases']['recover'];(self.folder/e['stdout_file']).write_text('{}');self.reject()
    def test_reused_phase_artifact(self):self.report['cases'][0]['v2']['phases']['recover']=self.report['cases'][0]['v2']['phases']['commit'];self.reject()
    def test_missing_phase(self):del self.report['cases'][0]['v2']['phases']['retry'];self.reject()
    def test_false_completion(self):self.phase('commit',lambda v:v.update(committed=0));self.reject()
    def test_false_decoded_diagnostic(self):self.phase('recover',lambda v:v.update(decoded_blob_bytes=1));self.reject()
    def test_false_physical_diagnostic(self):self.phase('commit',lambda v:v.update(physical_journal_bytes=1));self.reject()
    def test_false_cursor(self):self.phase('recover',lambda v:v.update(next_index=999));self.reject()
    def test_false_retry(self):self.phase('retry',lambda v:v.update(retry_head_unchanged=False));self.reject()
    def test_limit_override(self):self.phase('create',lambda v:v['limits'].update(max_physical_journal_bytes=2**30));self.reject()
    def test_step_omission(self):self.phase('commit',lambda v:v.update(steps=[]));self.reject()
    def test_unknown_target_count_rejects(self):self.report['points']=[0];self.reject()
    def test_missing_head(self):p=self.folder/self.report['cases'][0]['v2']['directory']/'HEAD';p.unlink();self.reject()
    def test_committed_tamper(self):p=self.folder/self.report['cases'][0]['v2']['directory']/'JOURNAL';b=bytearray(p.read_bytes());b[-1]^=1;p.write_bytes(b);self.reject()
    def test_full_bounded_reader_rejects_tail(self):p=self.folder/self.report['cases'][0]['v2']['directory']/'JOURNAL';p.write_bytes(p.read_bytes()+b'garbage');self.reject()

class CodecVectorTests(unittest.TestCase):
    def test_independent_writer_vectors_decode_with_python(self):
        for line in (ROOT/'adapters/mdc_replay/tests/fixtures/replay-writer-v2.tsv').read_text().splitlines():
            if line.startswith('#'):continue
            name,value=line.split('\t');b=bytes.fromhex(value);raw,end=blob(b,0,16384,16384)
            self.assertEqual(end,len(b));self.assertEqual(h(raw),b[17:49])
    def test_bounded_output_and_stream_eof(self):
        import struct,zlib
        raw=b'A'*1024;enc=zlib.compress(raw)
        for n,data in [(1,enc),(1024,enc+b'x'),(1024,enc+enc)]:
            b=b'\1'+struct.pack('<QQ',n,len(data))+h(raw)+data
            with self.assertRaises(ValueError):blob(b,0,16384,16384)
    def test_raw_length_and_tag(self):
        import struct
        for tag,n in [(0,2),(2,1)]:
            b=bytes([tag])+struct.pack('<QQ',n,1)+h(b'x')+b'x'
            with self.assertRaises(ValueError):blob(b,0,16384,16384)

if __name__=='__main__':unittest.main()
