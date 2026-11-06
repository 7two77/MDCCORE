import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import zipfile
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from exact_work import baseline,check_pair
from capacity_report import read_json,sha


class ExactWorkTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.context=baseline();cls.old=cls.context.__enter__()

    @classmethod
    def tearDownClass(cls):
        cls.context.__exit__(None,None,None)

    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.folder=Path(self.temp.name)/'pair';shutil.copytree(os.environ['MDC_EXACT_WORK_DIR'],self.folder)
        self.receipt=read_json(self.folder/'pair.json')

    def save(self):
        (self.folder/'pair.json').write_text(json.dumps(self.receipt))

    def reject(self):
        self.save()
        with self.assertRaises((ValueError,OSError,KeyError,TypeError,zipfile.BadZipFile)):
            check_pair(self.folder,self.old)

    def test_real_pair_passes(self):
        self.assertEqual(check_pair(self.folder,self.old)['comparison']['acceptance'],'PASS')

    def test_unknown_baseline_is_rejected(self):
        self.receipt['comparison']['baseline_commit']='0'*40;self.reject()

    def test_old_fingerprint_cannot_be_relabelled_current(self):
        self.receipt['comparison']['baseline_source_sha256']=self.receipt['comparison']['candidate_source_sha256'];self.reject()

    def test_candidate_fingerprint_cannot_be_relabelled_old(self):
        self.receipt['comparison']['candidate_source_sha256']=self.receipt['comparison']['baseline_source_sha256'];self.reject()

    def test_failure_cannot_be_hidden(self):
        self.receipt['comparison']['cases'][-1]['outcome']='Completed';self.reject()

    def test_missing_archive_is_rejected(self):
        (self.folder/'baseline.zip').unlink();self.reject()

    def test_modified_archive_is_rejected(self):
        with (self.folder/'candidate.zip').open('ab') as f:f.write(b'changed')
        self.reject()

    def test_forged_summary_is_rejected(self):
        self.receipt['comparison']['candidate_summary'][0]['metrics']['journal_bytes']['median']=0;self.reject()

    def test_archive_path_escape_is_rejected(self):
        self.receipt['archives']['baseline']['file']='../outside.zip';self.reject()

    def test_resealed_zip_cannot_hide_changed_source(self):
        path=self.folder/'candidate.zip'
        with zipfile.ZipFile(path) as z:entries=[(n,z.read(n)) for n in z.namelist()]
        path.unlink()
        with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_DEFLATED) as z:
            for n,data in entries:z.writestr(n,data+b'x' if n=='source.csv' else data)
        self.receipt['archives']['candidate']['sha256']=sha(path);self.reject()
