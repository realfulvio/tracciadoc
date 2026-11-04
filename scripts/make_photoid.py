"""Ritratti disegnati (nessuna persona reale) per le prove delle fototessere.

fototessera-sintetica.png: volto frontale unico su sfondo uniforme (atteso: segnalata).
ritratto-sfondo-vario.png: lo stesso volto su sfondo d'ambiente e piu piccolo (atteso: non segnalato).
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

OUT = Path(__file__).resolve().parents[1] / "fixtures" / "corpus"


def portrait(w=350, h=450, bg=(225, 228, 232), busy=False):
    im = Image.new("RGB", (w, h), bg)
    d = ImageDraw.Draw(im)
    if busy:
        for i in range(14):
            c = (40 + i * 15, 200 - i * 9, 90 + (i % 3) * 50)
            d.rectangle([0, i * 32, 70, i * 32 + 32], fill=c)
            d.rectangle([w - 70, i * 32, w, i * 32 + 32], fill=(c[2], c[0], c[1]))
    cx = w // 2
    skin, hair = (224, 172, 140), (60, 40, 30)
    d.ellipse([cx - 150, h * 0.90, cx + 150, h * 1.2], fill=(40, 60, 110))
    d.rectangle([cx - 30, h * 0.55, cx + 30, h * 0.75], fill=skin)
    d.ellipse([cx - 62, h * 0.18, cx + 62, h * 0.60], fill=skin)
    d.ellipse([cx - 64, h * 0.16, cx + 64, h * 0.34], fill=hair)
    for dx in (-28, 28):
        d.ellipse([cx + dx - 14, h * 0.34, cx + dx + 14, h * 0.38], fill=(255, 255, 255))
        d.ellipse([cx + dx - 6, h * 0.345, cx + dx + 6, h * 0.375], fill=(50, 30, 20))
        d.line([cx + dx - 18, h * 0.31, cx + dx + 18, h * 0.31], fill=hair, width=5)
    d.line([cx, h * 0.38, cx - 6, h * 0.48], fill=(190, 140, 110), width=3)
    d.line([cx - 8, h * 0.48, cx + 8, h * 0.48], fill=(190, 140, 110), width=3)
    d.arc([cx - 25, h * 0.48, cx + 25, h * 0.56], 20, 160, fill=(160, 60, 60), width=4)
    return im.filter(ImageFilter.GaussianBlur(1.2))


portrait().save(OUT / "fototessera-sintetica.png")
# Volto piccolo (circa 20% dell'altezza) su sfondo vario: non e una fototessera.
big = Image.new("RGB", (700, 900), (120, 160, 110))
big.paste(portrait(busy=True).resize((233, 300)), (233, 300))
big.save(OUT / "ritratto-sfondo-vario.png")
