#!/usr/bin/env python3
"""Fogli sintetici con piu documenti sulla stessa pagina (fotocopie da ufficio).

Solo dati finti (ESEMPIO / PROVA, numeri inventati). Genera in fixtures/multidoc/:
12 varianti a 150 e 200 dpi in JPEG, e a 150 dpi in PDF (un'immagine per pagina),
piu manifest.json con l'elenco. Tutte le varianti devono produrre un rilevamento.

Uso: python scripts/make_multidoc.py [cartella-uscita]
Serve Pillow e numpy; i font vengono cercati in C:/Windows/Fonts o in DejaVu.
"""
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageEnhance, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "fixtures" / "multidoc"
W0, H0 = 1240, 1754  # A4 a 150 dpi
RNG = np.random.default_rng(5)


def font(names, size):
    for base in ("C:/Windows/Fonts/", "/usr/share/fonts/truetype/dejavu/", "/usr/share/fonts/TTF/"):
        for n in names:
            try:
                return ImageFont.truetype(base + n, size)
            except OSError:
                pass
    return ImageFont.load_default()


class Sheet:
    def __init__(self, scale):
        self.k = scale
        self.im = Image.new("RGB", (round(W0 * scale), round(H0 * scale)), (250, 250, 247))
        self.d = ImageDraw.Draw(self.im)

    def s(self, v):
        return round(v * self.k)

    def box(self, x0, y0, x1, y1, fill, outline=(90, 90, 90), radius=0):
        r = (self.s(x0), self.s(y0), self.s(x1), self.s(y1))
        if radius:
            self.d.rounded_rectangle(r, radius=self.s(radius), fill=fill, outline=outline, width=2)
        else:
            self.d.rectangle(r, fill=fill, outline=outline, width=2)

    def text(self, x, y, t, kind="ser", size=22, fill=(40, 40, 40), anchor=None):
        names = {"ser": ("times.ttf", "DejaVuSerif.ttf"), "typ": ("cour.ttf", "DejaVuSansMono.ttf"),
                 "san": ("arialbd.ttf", "DejaVuSans-Bold.ttf"), "reg": ("arial.ttf", "DejaVuSans.ttf")}[kind]
        self.d.text((self.s(x), self.s(y)), t, font=font(names, self.s(size)), fill=fill, anchor=anchor)

    def photo(self, x0, y0, x1, y1):
        w, h = self.s(x1 - x0), self.s(y1 - y0)
        yy, xx = np.mgrid[0:h, 0:w]
        a = np.stack([150 + 50 * np.sin(xx / 60.0), 140 + 40 * np.cos(yy / 50.0), 135 + 30 * np.sin((xx + yy) / 80.0)], -1)
        a = a + RNG.normal(0, 6, (h, w, 3))
        self.im.paste(Image.fromarray(np.clip(a, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(3)),
                      (self.s(x0), self.s(y0)))


def carta_interno(sh, x0=60, y0=70):
    sh.box(x0, y0, x0 + 560, y0 + 790, (243, 240, 228))
    rows = [("Cognome", "ESEMPIO"), ("Nome", "PROVA"), ("nato il", "01/01/1950 a ESEMPIOPOLI"), ("Cittadinanza", "ITALIANA"),
            ("Residenza", "VIA PROVA 1 - ESEMPIOPOLI"), ("Stato civile", "CELIBE"), ("Professione", "IMPIEGATO")]
    y = y0 + 30
    for lab, val in rows:
        sh.text(x0 + 25, y, lab, "ser", 22, (70, 70, 70)); sh.text(x0 + 190, y - 2, val, "typ", 24, (20, 20, 20)); y += 52
    y += 14
    sh.text(x0 + 25, y, "CONNOTATI E CONTRASSEGNI SALIENTI", "ser", 22, (60, 60, 60)); y += 48
    for lab, val in (("Statura", "m. 1,70"), ("Capelli", "CASTANI"), ("Occhi", "AZZURRI"), ("Segni particolari", "NESSUNO")):
        sh.text(x0 + 25, y, lab, "ser", 22, (70, 70, 70)); sh.text(x0 + 230, y - 2, val, "typ", 24, (20, 20, 20)); y += 52


def carta_copertina(sh, x0=60, y0=70):
    sh.box(x0, y0, x0 + 560, y0 + 790, (236, 232, 215))
    sh.text(x0 + 280, y0 + 80, "REPUBBLICA ITALIANA", "ser", 30, anchor="mm")
    sh.text(x0 + 280, y0 + 150, "COMUNE DI ESEMPIO", "ser", 30, anchor="mm")
    sh.text(x0 + 280, y0 + 260, "CARTA D'IDENTITA", "ser", 40, (30, 30, 30), anchor="mm")
    sh.text(x0 + 280, y0 + 350, "N. AV 0000000", "typ", 34, (20, 20, 20), anchor="mm")


def tessera_fronte(sh, x0=120, y0=1040, w=1000, h=520):
    sh.box(x0, y0, x0 + w, y0 + h, (70, 120, 190), (30, 60, 120), 24)
    sh.text(x0 + 40, y0 + 30, "TESSERA SANITARIA", "san", 44, (255, 255, 255))
    sh.text(x0 + 40, y0 + 95, "REPUBBLICA ITALIANA", "san", 30, (255, 255, 255))
    y = y0 + 170
    for lab, val in (("Codice fiscale", "RSSMRA50A01H501Z"), ("Cognome", "ESEMPIO"), ("Nome", "PROVA"),
                     ("Data di nascita", "01/01/1950"), ("Scadenza", "01/01/2030")):
        sh.text(x0 + 40, y, lab, "san", 26, (230, 238, 250)); sh.text(x0 + 380, y, val, "san", 30, (255, 255, 255)); y += 66


def tessera_team(sh, x0=120, y0=1040, w=1000, h=520):
    sh.box(x0, y0, x0 + w, y0 + h, (190, 205, 225), (60, 90, 140), 24)
    sh.text(x0 + 40, y0 + 40, "TESSERA EUROPEA DI ASSICURAZIONE MALATTIA", "san", 30, (20, 40, 90))
    sh.text(x0 + 40, y0 + 120, "Questa tessera attesta il diritto alle prestazioni sanitarie", "san", 24)
    sh.text(x0 + 40, y0 + 170, "durante un soggiorno temporaneo nei Paesi dell'Unione Europea.", "san", 24)
    sh.text(x0 + 40, y0 + 260, "Numero di identificazione personale: 80380000000000000000", "san", 24)


def cie_fronte_retro(sh):
    for i, x0 in enumerate((60, 650)):
        sh.box(x0, 120, x0 + 530, 440, (225, 235, 232), (70, 100, 95), 18)
        if i == 0:
            sh.text(x0 + 25, 140, "REPUBBLICA ITALIANA", "san", 20, (40, 60, 60))
            sh.text(x0 + 25, 170, "CARTA DI IDENTITA' ELETTRONICA", "san", 18, (40, 60, 60))
            y = 215
            for lab, val in (("Cognome / Surname", "ESEMPIO"), ("Nome / Name", "PROVA"), ("Luogo e data di nascita", "ESEMPIOPOLI 01 01 1990"),
                             ("Sesso / Sex", "M"), ("Statura / Height", "170"), ("Cittadinanza", "ITA"), ("Data di scadenza", "01 01 2035")):
                sh.text(x0 + 25, y, lab, "reg", 14, (80, 90, 90)); sh.text(x0 + 250, y, val, "typ", 17, (20, 20, 20)); y += 30
        else:
            sh.text(x0 + 25, 150, "Codice fiscale RSSMRA90A01H501Z", "typ", 18, (20, 20, 20))
            sh.text(x0 + 25, 190, "Comune di ESEMPIOPOLI  N. CA00000AA", "typ", 18, (20, 20, 20))
            for j, line in enumerate(("IDITACA00000AA<<<<<<<<<<<<<<<", "9001010M3501017ITA<<<<<<<<<<<6", "ESEMPIO<<PROVA<<<<<<<<<<<<<<<<")):
                sh.text(x0 + 25, 300 + j * 38, line, "typ", 22, (20, 20, 20))
    tessera_fronte(sh, 120, 700, 1000, 520)


def patente(sh, x0=120, y0=100):
    sh.box(x0, y0, x0 + 1000, y0 + 560, (238, 232, 210), (110, 100, 70), 20)
    sh.text(x0 + 30, y0 + 20, "REPUBBLICA ITALIANA   PATENTE DI GUIDA", "san", 26, (40, 40, 40))
    y = y0 + 85
    for lab, val in (("1. Cognome", "ESEMPIO"), ("2. Nome", "PROVA"), ("3. Data e luogo di nascita", "01/01/1980 ESEMPIOPOLI (EP)"),
                     ("4a. Rilascio", "01/01/2020"), ("4b. Scadenza", "01/01/2030"), ("5. N. patente", "AB0000000A"), ("9. Categorie", "B")):
        sh.text(x0 + 30, y, lab, "reg", 21, (80, 80, 80)); sh.text(x0 + 420, y, val, "typ", 24, (20, 20, 20)); y += 58


def passaporto(sh, x0=120, y0=80):
    sh.box(x0, y0, x0 + 1000, y0 + 620, (232, 236, 244), (80, 90, 120), 16)
    sh.text(x0 + 30, y0 + 20, "REPUBBLICA ITALIANA   PASSAPORTO / PASSPORT", "san", 26, (30, 40, 80))
    sh.photo(x0 + 30, y0 + 80, x0 + 300, y0 + 440)
    y = y0 + 90
    for lab, val in (("Cognome / Surname", "ESEMPIO"), ("Nome / Given names", "PROVA"), ("Cittadinanza / Nationality", "ITALIANA"),
                     ("Data di nascita / Date of birth", "01 JAN 1980"), ("Data di scadenza / Date of expiry", "01 JAN 2030"), ("Passaporto n.", "YA0000000")):
        sh.text(x0 + 330, y, lab, "reg", 17, (80, 80, 100)); sh.text(x0 + 330, y + 22, val, "typ", 24, (20, 20, 20)); y += 60
    sh.text(x0 + 30, y0 + 500, "P<ITAESEMPIO<<PROVA<<<<<<<<<<<<<<<<<<<<<<<<<", "typ", 25, (20, 20, 20))
    sh.text(x0 + 30, y0 + 540, "YA00000000ITA8001019M3001017<<<<<<<<<<<<<<<04", "typ", 25, (20, 20, 20))


def with_tessera(draw_card, sh, **kw):
    draw_card(sh, **kw); tessera_fronte(sh)


LAYOUTS = {
    "carta-interno-tessera": lambda sh: (carta_interno(sh), sh.photo(700, 120, 1170, 760), tessera_fronte(sh)),
    "carta-copertina-team": lambda sh: (carta_copertina(sh), tessera_team(sh)),
    "cie-fronte-retro": cie_fronte_retro,
    "patente-tessera": lambda sh: (patente(sh), tessera_fronte(sh, 120, 900, 1000, 520)),
    "passaporto-tessera": lambda sh: (passaporto(sh), tessera_fronte(sh, 120, 1000, 1000, 520)),
}

# (nome, layout, trasformazione)
VARIANTS = [
    ("carta-interno-tessera", "carta-interno-tessera", None),
    ("carta-copertina-team", "carta-copertina-team", None),
    ("cie-fronte-retro", "cie-fronte-retro", None),
    ("patente-tessera", "patente-tessera", None),
    ("passaporto-tessera", "passaporto-tessera", None),
    ("carta-interno-inclinata-2", "carta-interno-tessera", ("skew", 2.5)),
    ("cie-inclinata-3", "cie-fronte-retro", ("skew", -3.0)),
    ("patente-inclinata-3", "patente-tessera", ("skew", 3.0)),
    ("patente-ruotata-90", "patente-tessera", ("rot", 90)),
    ("carta-copertina-contrasto-basso", "carta-copertina-team", ("lowcontrast", None)),
    ("passaporto-contrasto-basso", "passaporto-tessera", ("lowcontrast", None)),
    ("carta-interno-ruotata-90", "carta-interno-tessera", ("rot", 90)),
]


def finish(im, transform):
    if transform:
        kind, arg = transform
        if kind == "skew":
            im = im.rotate(arg, expand=True, fillcolor=(250, 250, 247), resample=Image.BICUBIC)
        elif kind == "rot":
            im = im.rotate(-arg, expand=True, fillcolor=(250, 250, 247))
        elif kind == "lowcontrast":
            im = ImageEnhance.Contrast(im.convert("L")).enhance(0.45).convert("RGB")
            im = ImageEnhance.Brightness(im).enhance(1.12)
    a = np.asarray(im, dtype=np.float32) + RNG.normal(0, 2, (im.height, im.width, 3))
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.6))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*"):
        old.unlink()
    manifest = []
    for name, layout, transform in VARIANTS:
        for dpi in (150, 200):
            sh = Sheet(dpi / 150)
            LAYOUTS[layout](sh)
            im = finish(sh.im, transform)
            f = f"{name}-{dpi}.jpg"
            im.save(OUT / f, quality=70, dpi=(dpi, dpi))
            manifest.append({"file": f, "positive": True})
            if dpi == 150:
                p = f"{name}-{dpi}.pdf"
                im.save(OUT / p, resolution=dpi)
                manifest.append({"file": p, "positive": True})
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    print(len(manifest), "file in", OUT)


if __name__ == "__main__":
    main()
