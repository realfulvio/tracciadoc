"""Fetch pinned official MSYS2 packages; extract runtime and licenses, never install."""
import hashlib, io, json, pathlib, tarfile, urllib.request
import zstandard
ROOT=pathlib.Path(__file__).resolve().parents[1]
CACHE=ROOT/'scratch'/'msys2'
DEST=ROOT/'runtime'/'ocr-current'
CACHE.mkdir(parents=True,exist_ok=True)
DEST.mkdir(parents=True,exist_ok=True)
manifest=json.loads((ROOT/'docs'/'runtime-lock.json').read_text())
for pkg in manifest:
    filename=pkg['url'].rsplit('/',1)[1]
    cache=CACHE/filename
    if not cache.exists():
        with urllib.request.urlopen(pkg['url']) as response:cache.write_bytes(response.read())
    data=cache.read_bytes()
    if hashlib.sha256(data).hexdigest()!=pkg['sha256']:raise RuntimeError('Package SHA256 mismatch: '+filename)
    print(filename,flush=True)
    with zstandard.ZstdDecompressor().stream_reader(io.BytesIO(data)) as reader:
        with tarfile.open(fileobj=reader,mode='r|') as archive:
            for member in archive:
                p=pathlib.PurePosixPath(member.name)
                if not member.isfile() or '..' in p.parts or p.is_absolute():continue
                if str(p).startswith('mingw64/bin/') and (p.suffix=='.dll' or p.name=='tesseract.exe'):
                    (DEST/p.name).write_bytes(archive.extractfile(member).read())
                elif str(p).startswith('mingw64/share/licenses/'):
                    out=DEST/'licenses'/pathlib.Path(*p.parts[3:])
                    out.parent.mkdir(parents=True,exist_ok=True)
                    out.write_bytes(archive.extractfile(member).read())
(DEST/'packages.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print('Runtime extracted; no system installation.',flush=True)
