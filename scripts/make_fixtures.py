"""Synthetic corpus. All names, dates and numbers are invented; no production data."""
from pathlib import Path
import io, json, zipfile
from PIL import Image, ImageDraw, ImageFont
from reportlab.pdfgen.canvas import Canvas
from reportlab.lib.utils import ImageReader
from reportlab.lib.pdfencrypt import StandardEncryption

ROOT=Path(__file__).resolve().parents[1]
DEST=ROOT/'fixtures'/'corpus'
DEST.mkdir(parents=True,exist_ok=True)
manifest=[]
TEXT={
 'carta':"CARTA DI IDENTITA\nCOGNOME: ESEMPIO\nNOME: PROVA SINTETICA\nDATA DI NASCITA: 01/01/2000\nSCADENZA: 01/01/2030\nNUMERO: AA00000AA",
 'passaporto':"PASSAPORTO\nSURNAME: EXAMPLE\nNAME: SYNTHETIC TEST\nDATE OF BIRTH: 01/01/2000\nEXPIRY: 01/01/2030\nNUMERO: AA0000000",
 'patente':"PATENTE DI GUIDA\nCOGNOME: ESEMPIO\nNOME: PROVA SINTETICA\nNASCITA: 01/01/2000\nRILASCIO: 01/01/2020\nSCADENZA: 01/01/2030\nNUMERO: AA0000000",
 'sanitaria':"TESSERA SANITARIA\nCOGNOME: ESEMPIO\nNOME: PROVA SINTETICA\nNASCITA: 01/01/2000\nSCADENZA: 01/01/2030\nCODICE FISCALE: RSSMRA80A01H501U",
}
def record(name,positive,cls,fmt,limited=False):
 manifest.append(dict(file=name,positive=positive,document_class=cls,format=fmt,limited=limited))
def image(text,low=False):
 im=Image.new('RGB',(1500,1000),'white');d=ImageDraw.Draw(im)
 font=ImageFont.truetype('C:/Windows/Fonts/arial.ttf',38)
 d.text((60,50),'FACSIMILE SINTETICO - NON VALIDO',fill='black',font=font)
 d.multiline_text((60,170),text,fill=(170,170,170) if low else 'black',font=font,spacing=22)
 return im
def pdf(name,text=None,img=None,encrypted=False):
 stream=io.BytesIO();c=Canvas(stream,pagesize=(595,842),encrypt=StandardEncryption('synthetic-only') if encrypted else None)
 c.setAuthor('TracciaDoc - fixture sintetica');c.setTitle('FACSIMILE SINTETICO - NON VALIDO')
 if text:
  c.setFont('Helvetica',13);y=780
  for line in ('FACSIMILE SINTETICO - NON VALIDO\n'+text).splitlines():c.drawString(45,y,line);y-=25
 if img is not None:c.drawImage(ImageReader(img),35,80,width=525,height=350)
 c.save();(DEST/name).write_bytes(stream.getvalue())
def office(name,text='',img=None):
 ext=Path(name).suffix
 if ext=='.docx':
  from docx import Document
  from docx.shared import Inches
  d=Document()
  d.core_properties.author='TracciaDoc';d.core_properties.last_modified_by='TracciaDoc'
  for line in text.splitlines():d.add_paragraph(line)
  if img is not None:
   b=io.BytesIO();img.save(b,format='PNG');b.seek(0);d.add_picture(b,width=Inches(6))
  d.save(DEST/name);return
 if ext=='.xlsx':
  from openpyxl import Workbook
  w=Workbook();s=w.active
  w.properties.creator='TracciaDoc';w.properties.lastModifiedBy='TracciaDoc'
  for row,line in enumerate(text.splitlines(),1):s.cell(row,1,line)
  w.save(DEST/name);return
 if ext=='.pptx':
  from pptx import Presentation
  p=Presentation();s=p.slides.add_slide(p.slide_layouts[5]);s.shapes.title.text='FACSIMILE SINTETICO'
  p.core_properties.author='TracciaDoc';p.core_properties.last_modified_by='TracciaDoc'
  from pptx.util import Inches
  s.shapes.add_textbox(Inches(1),Inches(1),Inches(8),Inches(5)).text_frame.text=text
  p.save(DEST/name);return
 if ext in ['.odt','.ods','.odp']:
  from odf.opendocument import OpenDocumentText,OpenDocumentSpreadsheet,OpenDocumentPresentation
  from odf.text import P
  from odf.table import Table,TableRow,TableCell
  from odf.draw import Page,Frame,TextBox
  if ext=='.odt':
   d=OpenDocumentText()
   for line in text.splitlines():d.text.addElement(P(text=line))
  elif ext=='.ods':
   d=OpenDocumentSpreadsheet();table=Table(name='Sintetico')
   for line in text.splitlines():
    row=TableRow();cell=TableCell(valuetype='string');cell.addElement(P(text=line));row.addElement(cell);table.addElement(row)
   d.spreadsheet.addElement(table)
  else:
   d=OpenDocumentPresentation();page=Page(name='Sintetico',masterpagename='Default');frame=Frame(x='1cm',y='1cm',width='20cm',height='12cm');box=TextBox()
   for line in text.splitlines():box.addElement(P(text=line))
   frame.addElement(box);page.addElement(frame);d.presentation.addElement(page)
  d.save(str(DEST/name));return
 raise ValueError('Unsupported fixture format')

for cls,text in TEXT.items():
 name=cls+'.pdf';pdf(name,text);record(name,True,cls,'PDF')
 name=cls+'.png';image(text).save(DEST/name);record(name,True,cls,'PNG')
for ext,cls in [('.docx','carta'),('.xlsx','sanitaria'),('.pptx','patente'),('.odt','passaporto'),('.ods','sanitaria'),('.odp','carta')]:
 name='documento'+ext;office(name,TEXT[cls]);record(name,True,cls,ext[1:].upper())
for deg in [90,180,270]:
 name=f'generico-ruotato-{deg}.jpg';image(TEXT['passaporto']).rotate(deg,expand=True).save(DEST/name,quality=92);record(name,True,'passaporto','JPEG')
image(TEXT['sanitaria'],True).save(DEST/'poco-contrasto.bmp');record('poco-contrasto.bmp',True,'sanitaria','BMP')
pdf('misto.pdf','Nota ordinaria: documento sintetico incorporato.',image(TEXT['patente']));record('misto.pdf',True,'patente','PDF misto')
pdf('raster.pdf',img=image(TEXT['carta']));record('raster.pdf',True,'carta','PDF raster')
office('immagine.docx','Nota ordinaria.',image(TEXT['carta']));record('immagine.docx',True,'carta','DOCX immagine')
# La regola 0.2.4 apre i .bin solo da 20 KB in su: un PNG rinominato lo supera, un PDF testuale no.
image(TEXT['carta']).save(DEST/'nome-errato.bin',format='PNG');record('nome-errato.bin',True,'carta','PNG firma')
image(TEXT['patente']).save(DEST/'senza-estensione',format='PNG');record('senza-estensione',True,'patente','PNG firma')
image('APPUNTI DI RIUNIONE\nAcquisto materiali\nVerifica preventivi\nProssimo incontro').save(DEST/'appunti.png');record('appunti.png',False,'negativo','PNG')
pdf('ordinario.pdf','Appunti di riunione. Acquisto materiali e verifica preventivi.');record('ordinario.pdf',False,'negativo','PDF')
(DEST/'corrotto.pdf').write_bytes(b'%PDF-1.7\nBROKEN');record('corrotto.pdf',False,'problema','PDF corrotto',True)
pdf('cifrato.pdf',TEXT['carta'],encrypted=True);record('cifrato.pdf',True,'carta','PDF cifrato',True)
(DEST/'legacy.doc').write_bytes(bytes.fromhex('d0cf11e0a1b11ae1')+b'fixture-not-a-document');record('legacy.doc',False,'problema','DOC non supportato',True)
# Photo-like images without text (smooth upscaled noise) and a rotated document inside a larger photo.
from PIL import ImageFilter
import random
random.seed(11)
def photo(w,h):
 im=Image.effect_noise((w//8,h//8),60).convert('RGB').resize((w,h),Image.BICUBIC)
 return Image.merge('RGB',[c.point(lambda v,k=k:(v+k*40)%256) for k,c in enumerate(im.split())]).filter(ImageFilter.GaussianBlur(3))
for i,(w,h) in enumerate([(1600,1200),(2400,1800)],1):
 photo(w,h).save(DEST/f'foto-senza-testo-{i}.jpg',quality=88);record(f'foto-senza-testo-{i}.jpg',False,'negativo','JPEG foto senza testo',True)
image(TEXT['carta']).rotate(90,expand=True).save(DEST/'carta-ruotata-90.png');record('carta-ruotata-90.png',True,'carta','PNG ruotato')
scene=photo(2400,1800);doc=image(TEXT['patente']).resize((900,600)).rotate(270,expand=True);scene.paste(doc,(1200,500))
scene.save(DEST/'documento-fotografato.jpg',quality=90);record('documento-fotografato.jpg',True,'patente','JPEG foto con documento')
with zipfile.ZipFile(DEST/'archivio.zip','w') as z:z.writestr('../../outside.txt','synthetic')
record('archivio.zip',False,'problema','ZIP non aperto',True)
(ROOT/'fixtures'/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print(f'{len(manifest)} synthetic fixtures generated.')
