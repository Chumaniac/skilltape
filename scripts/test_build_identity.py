import importlib.util,json,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('package_release',ROOT/'scripts/package_release.py');pack=importlib.util.module_from_spec(spec);spec.loader.exec_module(pack)
class BuildIdentityTests(unittest.TestCase):
 def fixture(self):
  catalog=json.loads((ROOT/'product-capabilities.json').read_text())
  return {'schema':'skilltape.build-info/v1','version':'0.4.1','sourceRevision':'a'*40,'dirty':False,'channel':'source','capabilities':catalog['capabilities']}
 def test_mixed_source_or_dirty_component_is_rejected(self):
  info=self.fixture();cli={**info,'target':'aarch64-apple-darwin'}
  pack.validate_identities('0.4.1','aarch64-apple-darwin',cli,cli,info)
  for key,value in [('sourceRevision','b'*40),('dirty',True),('capabilities',[])]:
   with self.assertRaises(ValueError):pack.validate_identities('0.4.1','aarch64-apple-darwin',cli,cli,{**info,key:value})
 def test_unknown_source_wrong_target_and_channel_are_not_installation_proof(self):
  info=self.fixture();cli={**info,'target':'aarch64-apple-darwin'}
  for broken in [{**cli,'sourceRevision':None},{**cli,'target':'x86_64-apple-darwin'},{**cli,'channel':'release'}]:
   with self.assertRaises(ValueError):pack.validate_identities('0.4.1','aarch64-apple-darwin',broken,cli,info)
if __name__=='__main__':unittest.main()
