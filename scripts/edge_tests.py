"""Isolated synthetic error/limit tests for the portable Windows release."""
import ctypes, json, pathlib, shutil, subprocess, time
ROOT=pathlib.Path(__file__).resolve().parents[1]
PKG=ROOT/'dist'/'TracciaDoc';EXE=PKG/'TracciaDoc.exe'
OUT=ROOT/'scratch'/'edges'/str(time.time_ns());OUT.mkdir(parents=True)
def scan(name,source,flags=()):
 target=OUT/(name+'.pdf')
 subprocess.run([str(EXE),'--scan',str(source),str(target),'--demo',*flags],check=True,timeout=90)
 result=json.loads(target.with_suffix('.json').read_text())
 print(name,result['encountered'],result['analyzed'],result['partial'],result['skipped'],result['incomplete'],flush=True)
 return result

source=OUT/'one';source.mkdir();shutil.copy2(PKG/'prove-sintetiche'/'appunti.png',source/'nota.png')
assert not scan('negative',source)['incomplete']
shutil.copy2(PKG/'prove-sintetiche'/'carta.png',source/'documento.png')
assert len(scan('positive',source)['findings'])==1
assert scan('max-files',source,['--max-files','1'])['incomplete']

# An exclusive read handle produces a real Windows sharing violation. No ACL/system setting changes.
kernel=ctypes.WinDLL('kernel32',use_last_error=True)
kernel.CreateFileW.restype=ctypes.c_void_p
kernel.CreateFileW.argtypes=[ctypes.c_wchar_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p]
handle=kernel.CreateFileW(str(source/'documento.png'),0x80000000,0,None,3,0,None)
assert handle not in [None,ctypes.c_void_p(-1).value]
try:
 result=scan('locked',source)
 assert result['skipped']==1 and result['incomplete']
finally:
 kernel.CloseHandle.argtypes=[ctypes.c_void_p];kernel.CloseHandle(handle)

# Read denied only on a synthetic file, and reverted in finally.
denied=OUT/'denied';denied.mkdir();p=denied/'synthetic.png';p.write_text('synthetic only')
subprocess.run(['icacls',str(p),'/deny','*S-1-1-0:(R)'],check=True,capture_output=True)
try:
 result=scan('denied',denied);assert result['incomplete'] and result['skipped']==1
finally:
 subprocess.run(['icacls',str(p),'/remove:d','*S-1-1-0'],check=True,capture_output=True)

large=OUT/'large';large.mkdir();(large/'large.pdf').write_bytes(b'x'*1_100_000)
assert scan('size',large,['--max-mb','1'])['incomplete']
# A 3x3 mosaic of a text image: OCR well above one second even with a single confident pass.
from PIL import Image
note=Image.open(PKG/'prove-sintetiche'/'appunti.png');mosaic=Image.new('RGB',(note.width*3,note.height*3),'white')
for i in range(9):mosaic.paste(note,((i%3)*note.width,(i//3)*note.height))
images=OUT/'images';images.mkdir();mosaic.save(images/'a.png')
shutil.copy2(PKG/'prove-sintetiche'/'carta.png',images/'z.png')
result=scan('timeout',images,['--timeout-secs','1'])
assert result['incomplete'] and len(result['findings'])==1,'Timeout must recover and continue'
# Deterministic memory pressure: a 5600x5600 RGB raster decodes to ~94 MB, within the 128 MiB decoder
# limit but above a 64 MiB worker Job limit, whatever the OCR footprint of the machine.
heavy=OUT/'memory';heavy.mkdir();Image.new('RGB',(5600,5600),'white').save(heavy/'a.png')
shutil.copy2(PKG/'prove-sintetiche'/'carta.png',heavy/'z.png')
result=scan('memory',heavy,['--memory-mb','64'])
assert result['incomplete'] and len(result['findings'])==1,'Memory failure must recover and continue'
assert scan('missing',OUT/'absent')['incomplete']

# Directory junction outside selected root is counted and not followed.
links=OUT/'links';links.mkdir()
subprocess.run(['powershell.exe','-NoProfile','-Command',f"New-Item -ItemType Junction -Path '{links / 'outside'}' -Target '{source}' | Out-Null"],check=True,capture_output=True)
assert scan('junction',links)['encountered']==0
assert not (OUT.parent/'outside.txt').exists()

# No recognized content or file paths appear in the audit stream.
audit=(pathlib.Path.home()/'AppData'/'Local'/'TracciaDoc'/'audit.jsonl').read_text(encoding='utf-8')
assert 'RSSMRA80A01H501U' not in audit and str(source) not in audit
(OUT/'PASS.txt').write_text('Synthetic error/limit tests passed. ACL restored.\n')
print('All edge checks passed; synthetic ACL restored.',flush=True)
