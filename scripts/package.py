"""Package current release and exact imported OCR DLL closure; never install anything."""
from pathlib import Path
import hashlib, json, shutil, subprocess
import pefile
ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'dist'/'TracciaDoc'
DEST.mkdir(parents=True,exist_ok=True)
shutil.copy2(ROOT/'target'/'release'/'TracciaDoc.exe',DEST/'TracciaDoc.exe')
ocr=ROOT/'runtime'/'ocr-current'
out=DEST/'runtime'/'ocr';out.mkdir(parents=True,exist_ok=True)
pending=['libtesseract-5.5.dll'];seen=set()
available={p.name.lower():p for p in ocr.glob('*.dll')}
system=[]
while pending:
 name=pending.pop().lower()
 if name in seen:continue
 seen.add(name)
 if name not in available:
  system.append(name);continue
 path=available[name];shutil.copy2(path,out/path.name)
 pe=pefile.PE(str(path),fast_load=True)
 pe.parse_data_directories(directories=[pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_IMPORT']])
 for entry in getattr(pe,'DIRECTORY_ENTRY_IMPORT',[]):pending.append(entry.dll.decode('ascii'))
 pe.close()
data=out/'tessdata';data.mkdir(exist_ok=True)
for lang in ['ita','eng','osd']:shutil.copy2(ROOT/'runtime'/'models'/f'{lang}.traineddata',data)
shutil.copy2(ROOT/'runtime'/'models'/'LICENSE',data/'LICENSE.txt')
shutil.copytree(ocr/'licenses',out/'licenses',dirs_exist_ok=True)
shutil.copy2(ocr/'packages.json',out/'packages.json')
pdf=DEST/'runtime'/'bin';pdf.mkdir(parents=True,exist_ok=True)
shutil.copy2(ROOT/'runtime'/'bin'/'pdfium.dll',pdf)
shutil.copytree(ROOT/'runtime'/'licenses',DEST/'licenze'/'pdfium',dirs_exist_ok=True)
shutil.copy2(ROOT/'runtime'/'LICENSE',DEST/'licenze'/'PDFium-LICENSE.txt')
shutil.copy2(ROOT/'LICENSE',DEST/'LICENSE.txt')
shutil.copytree(ROOT/'assets'/'ui',DEST/'licenze'/'ui-kit',ignore=shutil.ignore_patterns('*.ttf','*.png','*.ico'),dirs_exist_ok=True)
shutil.copytree(ROOT/'fixtures'/'corpus',DEST/'prove-sintetiche',dirs_exist_ok=True)
shutil.copy2(ROOT/'fixtures'/'manifest.json',DEST/'manifest-prove.json')
metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'],cwd=ROOT))
listing=[]
for pkg in metadata['packages']:
 listing.append(f"{pkg['name']} {pkg['version']}: {pkg.get('license','SEE LICENSE')} | {pkg.get('repository','')}")
 source=Path(pkg['manifest_path']).parent
 for p in source.iterdir():
  if p.is_file() and p.name.upper().startswith(('LICENSE','COPYING','NOTICE','UNLICENSE')):
   d=DEST/'licenze'/'rust'/f"{pkg['name']}-{pkg['version']}";d.mkdir(parents=True,exist_ok=True);shutil.copy2(p,d/p.name)
 if pkg['name']=='epaint_default_fonts':
  for p in (source/'fonts').glob('*.txt'):
   d=DEST/'licenze'/'fonts';d.mkdir(parents=True,exist_ok=True);shutil.copy2(p,d/p.name)
(DEST/'licenze'/'dipendenze-rust.txt').write_text('\n'.join(sorted(listing)),encoding='utf-8')
for name in ['ISTRUZIONI.txt','COMPONENTI.md']:
 if (ROOT/'docs'/name).exists():shutil.copy2(ROOT/'docs'/name,DEST/name)
if (ROOT/'scratch'/'results'/'esempio-consegna.pdf').exists():shutil.copy2(ROOT/'scratch'/'results'/'esempio-consegna.pdf',DEST/'Esempio-verbale.pdf')
manifest=[]
for p in sorted(DEST.rglob('*')):
 if p.is_file() and p.name!='SHA256SUMS.txt':manifest.append(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+p.relative_to(DEST).as_posix())
(DEST/'SHA256SUMS.txt').write_text('\n'.join(manifest),encoding='utf-8')
print(json.dumps({'files':len(manifest),'size_bytes':sum(p.stat().st_size for p in DEST.rglob('*') if p.is_file()),'ocr_dlls':len(seen)-len(set(system)),'system_imports':sorted(set(system))},indent=2))
