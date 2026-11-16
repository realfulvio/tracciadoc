"""Regressioni su fixture sintetiche per il pacchetto Windows, senza dati reali.

Non modifica impostazioni, servizi o ACL. Ogni corsa usa una cartella nuova in scratch.
Uso: python scripts/collaudo_regressions.py [cartella-pacchetto]
"""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import time
import zipfile

from PIL import Image
import pypdfium2 as pdfium
from reportlab.pdfgen import canvas
from reportlab.lib.utils import ImageReader

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT / 'dist/TracciaDoc'
EXE = PACKAGE / 'TracciaDoc.exe'
OUT = ROOT / 'scratch' / 'collaudo-regressions' / str(time.time_ns())
OUT.mkdir(parents=True)
results = []


def scan(name, source, *flags):
    target = OUT / (name + '.pdf')
    before = {p.relative_to(source).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in source.rglob('*') if p.is_file()}
    subprocess.run([str(EXE), '--scan', str(source), str(target), '--workers', '1', *flags],
                   check=True, timeout=300)
    result = json.loads(target.with_suffix('.json').read_text(encoding='utf-8-sig'))
    assert '' not in result['timing_by_format'], 'Summary JSON contiene una chiave vuota'
    assert 'coverage_gaps' not in result and 'roots' not in result
    assert not any('path' in f for f in result['findings']), 'Il riepilogo CLI deve restare senza percorsi'
    # Exercise the actual consumer which rejected the earlier summary.
    command = "$ErrorActionPreference='Stop'; Get-Content -Raw -LiteralPath '" + str(target.with_suffix('.json')).replace("'", "''") + "' | ConvertFrom-Json | Out-Null"
    subprocess.run(['powershell.exe', '-NoProfile', '-Command', command], check=True, capture_output=True)
    after = {p.relative_to(source).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in source.rglob('*') if p.is_file()}
    assert before == after, 'La scansione ha modificato una fixture'
    doc = pdfium.PdfDocument(str(target))
    text = '\n'.join(doc[i].get_textpage().get_text_range() for i in range(len(doc)))
    doc.close()
    results.append({'case': name, 'summary': result})
    print(name, 'findings', len(result['findings']), 'incomplete', result['incomplete'], flush=True)
    return result, text


def folder(name):
    path = OUT / name
    path.mkdir()
    return path


containers = folder('containers-input')
with zipfile.ZipFile(containers / 'synthetic.zip', 'w') as z:
    z.writestr('document.txt', 'CARTA DI IDENTITA - FACSIMILE SINTETICO')
(containers / 'synthetic.eml').write_text('MIME-Version: 1.0\r\nContent-Type: text/plain\r\n\r\nFACSIMILE SINTETICO')
result, text = scan('containers', containers)
assert result['incomplete'] and result['ignored_type'] == 0
assert 'synthetic.zip' in text and 'synthetic.eml' in text
_, demo_text = scan('containers-demo', containers, '--demo')
assert 'CORPUS-SINTETICO' in demo_text and str(containers) not in demo_text

unknown = folder('unknown-input')
(unknown / 'blob.dat').write_bytes(b'not a supported document')
result, _ = scan('unknown', unknown)
assert result['ignored_type'] == 1 and not result['incomplete']

empty = folder('empty-input')
(empty / 'empty.pdf').write_bytes(b'')
assert scan('empty', empty)[0]['incomplete']

small = folder('small-input')
Image.new('RGB', (32, 32), 'white').save(small / 'tiny.png')
(small / 'tiny.tmp').write_bytes((small / 'tiny.png').read_bytes())
(small / 'tiny.png').unlink()
assert (small / 'tiny.tmp').stat().st_size < 20 * 1024
result, _ = scan('small', small)
assert result['ignored_type'] == 0 and 'Png' in result['timing_by_format']

renamed = folder('renamed-input')
positive = (PACKAGE / 'prove-sintetiche/carta.png').read_bytes()
(renamed / 'scan.jpg.txt').write_bytes(positive)
(renamed / 'scan.jpg.crdownload').write_bytes(positive)
assert len(scan('renamed', renamed)[0]['findings']) == 2

ads = folder('ads-input')
host = ads / 'ordinary.txt'
host.write_text('Synthetic host file')
stream = Path(str(host) + ':synthetic.png')
stream.write_bytes(positive)
Path(str(ads) + ':synthetic.pdf').write_bytes(b'SYNTHETIC DIRECTORY STREAM')
result, text = scan('ads', ads)
assert result['incomplete'] and any('Flussi alternativi NTFS presenti' in x for x in result['issues'])
assert 'ordinary.txt' in text and stream.read_bytes() == positive

unicode = folder('unicode-input')
for name in ['\u0416.png', '\u4e2d\u6587.png', '\U0001f600.png']:
    (unicode / name).write_bytes(positive)
result, text = scan('unicode', unicode)
assert len(result['findings']) == 3
for escaped in ['\\u{416}', '\\u{4e2d}\\u{6587}', '\\u{1f600}']:
    assert escaped in text, ('Percorso Unicode perso nel PDF', escaped)
_, omitted = scan('unicode-omitted', unicode, '--omit-paths')
assert str(unicode) not in omitted and 'unicode-input' not in omitted

text_only = folder('text-only-input')
pdf = canvas.Canvas(str(text_only / 'ordinary.pdf'))
for page in range(3):
    for row in range(20):
        pdf.drawString(40, 800 - row * 30, f'Nota sintetica ordinaria, pagina {page + 1}, riga {row + 1}.')
    pdf.showPage()
pdf.save()
result, _ = scan('text-only', text_only)
assert not result['incomplete'] and not result['findings']
assert result['pages_ocr'] == 0 and result['timing']['pdf_text_only_pages'] == 3

mixed = folder('mixed-input')
pdf = canvas.Canvas(str(mixed / 'mixed.pdf'))
pdf.drawString(40, 800, 'Pratica sintetica: immagine allegata')
pdf.drawImage(ImageReader(str(PACKAGE / 'prove-sintetiche/carta.png')), 40, 420,
              width=490, height=320, preserveAspectRatio=True)
pdf.save()
result, _ = scan('mixed', mixed)
assert result['pages_ocr'] > 0 and result['findings'], 'Un PDF misto deve ancora passare da OCR'

(OUT / 'results.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
(OUT / 'PASS.txt').write_text('Regressioni del pacchetto superate su sole fixture sintetiche.\n')
print('PASS', OUT, flush=True)
