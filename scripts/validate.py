"""Run only the labelled synthetic corpus against the packaged EXE, no real documents.
python scripts/validate.py [package-folder]
"""
from pathlib import Path
import collections, hashlib, json, os, subprocess, sys, time
ROOT=Path(__file__).resolve().parents[1]
PKG=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else ROOT/'dist'/'TracciaDoc'
OUT=ROOT/'scratch'/'validation';OUT.mkdir(parents=True,exist_ok=True)
exe=PKG/'TracciaDoc.exe';corpus=PKG/'prove-sintetiche'
manifest=json.loads((ROOT/'fixtures'/'manifest.json').read_text())
hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in corpus.iterdir() if p.is_file()}
settings={'deep':True,'cpu_only':False,'max_mb':64,'max_pages':40,'timeout_secs':120,'max_entries':1000000,'memory_mb':768,'workers':1,'prefilter':True}
expected_findings=sum(1 for item in manifest if item['positive'] and not item['limited'])
results=[]
for method in ['generic','avx2']:
 env=os.environ.copy();env.update(DOTPRODUCT=method,OMP_THREAD_LIMIT='1',OMP_NUM_THREADS='1')
 start=time.perf_counter()
 p=subprocess.Popen([str(exe),'--worker'],cwd=PKG/'runtime'/'ocr',env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,encoding='utf-8',creationflags=0x08000000)
 per_file=[];matrix=collections.defaultdict(lambda:collections.Counter())
 for item in manifest:
  req={'path':str(corpus/item['file']),'settings':settings|{'cpu_only':method=='generic'}}
  t=time.perf_counter();p.stdin.write(json.dumps(req)+'\n');p.stdin.flush()
  response=p.stdout.readline()
  if not response:raise RuntimeError('Worker stopped')
  a=json.loads(response);elapsed=time.perf_counter()-t
  positive=bool(a['reasons'])
  key=item['document_class']+' / '+item['format']
  if item['limited']:
   assert a['issues'],item
   matrix[key]['limited']+=1
  else:
   label='TP' if positive and item['positive'] else 'FN' if item['positive'] else 'FP' if positive else 'TN'
   matrix[key][label]+=1
  per_file.append({'fixture':item['file'],'found':positive,'format':a['format'],'reasons':a['reasons'],'issues':a['issues'],'seconds':elapsed,'backend':a['backend']})
 p.stdin.close();p.wait(timeout=10)
 results.append({'method':method,'seconds':time.perf_counter()-start,'matrix':dict(matrix),'files':per_file})
 print(method,round(results[-1]['seconds'],3),dict(sum((v for v in matrix.values()),collections.Counter())),flush=True)
assert [f['reasons'] for f in results[0]['files']]==[f['reasons'] for f in results[1]['files']], 'Quality mismatch between CPU paths'
assert hashes=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in corpus.iterdir() if p.is_file()},'Input files changed'
for result in results:
 for counts in result['matrix'].values():assert not counts.get('FN') and not counts.get('FP'), counts
(OUT/'quality.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
print('Quality equivalence and read-only input hashes verified.',flush=True)

# End-to-end CLI scans include enumeration, process isolation, OCR and PDF production.
bench=[]
stamp=str(time.time_ns())
for label,flags in [('auto-cold',[]),('auto-warm',[]),('cpu-generic',['--cpu']),('cancelled',['--cancel-after-ms','250'])]:
 target=OUT/f'{stamp}-{label}.pdf'
 t=time.perf_counter();subprocess.run([str(exe),'--scan',str(corpus),str(target),'--demo',*flags],check=True,timeout=180)
 a=json.loads(target.with_suffix('.json').read_text())
 bench.append({'run':label,'wall_seconds':time.perf_counter()-t,**a})
 print(label,round(a['seconds'],3),'s',a['encountered'],'files',len(a['findings']),'findings',flush=True)
 if label=='cancelled':assert a['interrupted'] and a['incomplete']
 if label in ['auto-cold','auto-warm','cpu-generic']:
  assert a['encountered']==len(manifest) and len(a['findings'])==expected_findings and a['incomplete']
(OUT/'benchmark.json').write_text(json.dumps(bench,indent=2),encoding='utf-8')
assert hashes=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in corpus.iterdir() if p.is_file()}
print('End-to-end package checks passed.',flush=True)
assert subprocess.run([str(exe),'--scan',str(corpus),str(OUT/'quick-rejected.pdf'),'--quick'],capture_output=True).returncode!=0
assert not (OUT/'quick-rejected.pdf').exists()
print('Legacy quick option rejected; all new scans use full coverage.',flush=True)
