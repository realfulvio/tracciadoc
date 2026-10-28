use crate::{
    model::{Analysis, Request},
    rules,
};
use image::DynamicImage;
use pdfium_render::prelude::*;
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    fs::OpenOptions,
    io::{BufRead, Read, Write},
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};

pub fn runtime() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("runtime")
}
type Create = unsafe extern "C" fn() -> *mut c_void;
type Init = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type SetImage = unsafe extern "C" fn(*mut c_void, *const u8, c_int, c_int, c_int, c_int);
type GetText = unsafe extern "C" fn(*mut c_void) -> *mut c_char;
type DeleteText = unsafe extern "C" fn(*const c_char);
type Delete = unsafe extern "C" fn(*mut c_void);
type SetVariable = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;

// Loaded only in an expendable worker; one engine reused for the entire scan.
struct Ocr {
    _lib: libloading::Library,
    api: *mut c_void,
    image: SetImage,
    text: GetText,
    free: DeleteText,
    delete: Delete,
    backend: String,
}
impl Ocr {
    unsafe fn load(cpu_only: bool) -> Result<Self, String> {
        unsafe {
            let lib = libloading::Library::new(runtime().join("ocr/libtesseract-5.5.dll"))
                .map_err(|_| "OCR: libreria non disponibile")?;
            let create: Create = *lib
                .get(b"TessBaseAPICreate\0")
                .map_err(|_| "OCR: API assente")?;
            let init: Init = *lib
                .get(b"TessBaseAPIInit3\0")
                .map_err(|_| "OCR: API assente")?;
            let delete: Delete = *lib
                .get(b"TessBaseAPIDelete\0")
                .map_err(|_| "OCR: API assente")?;
            let image = *lib
                .get(b"TessBaseAPISetImage\0")
                .map_err(|_| "OCR: API assente")?;
            let text = *lib
                .get(b"TessBaseAPIGetUTF8Text\0")
                .map_err(|_| "OCR: API assente")?;
            let free = *lib
                .get(b"TessDeleteText\0")
                .map_err(|_| "OCR: API assente")?;
            let set: SetVariable = *lib
                .get(b"TessBaseAPISetVariable\0")
                .map_err(|_| "OCR: API assente")?;
            let get: unsafe extern "C" fn(*mut c_void, *const c_char) -> *const c_char = *lib
                .get(b"TessBaseAPIGetStringVariable\0")
                .map_err(|_| "OCR: API assente")?;
            let api = create();
            if api.is_null() {
                return Err("OCR: memoria insufficiente".into());
            }
            let data = CString::new(runtime().join("ocr/tessdata").to_string_lossy().as_bytes())
                .map_err(|_| "OCR: percorso non valido")?;
            if init(api, data.as_ptr(), c"ita+eng".as_ptr()) != 0 {
                delete(api);
                return Err("OCR: modelli ita+eng non disponibili".into());
            }
            set(api, c"user_defined_dpi".as_ptr(), c"200".as_ptr());
            // CPU-only forbids GPU providers, not safe SIMD internal to the CPU OCR build.
            let _ = cpu_only;
            let method = get(api, c"dotproduct".as_ptr());
            let method = if method.is_null() {
                "non confermato".into()
            } else {
                CStr::from_ptr(method).to_string_lossy().into_owned()
            };
            let backend = format!(
                "CPU / Tesseract 5.5.3 LSTM ita+eng; dotproduct effettivo: {method}; nessuna GPU"
            );
            Ok(Self {
                _lib: lib,
                api,
                image,
                text,
                free,
                delete,
                backend,
            })
        }
    }
    fn recognize(&self, img: &DynamicImage) -> Result<String, String> {
        let rgb = img.to_rgb8();
        unsafe {
            (self.image)(
                self.api,
                rgb.as_ptr(),
                rgb.width() as i32,
                rgb.height() as i32,
                3,
                (rgb.width() * 3) as i32,
            );
            let ptr = (self.text)(self.api);
            if ptr.is_null() {
                return Err("OCR: riconoscimento fallito".into());
            }
            let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            (self.free)(ptr);
            if s.len() > 4_000_000 {
                return Err("OCR: limite testo superato".into());
            }
            Ok(s)
        }
    }
}
impl Drop for Ocr {
    fn drop(&mut self) {
        unsafe { (self.delete)(self.api) }
    }
}

pub struct Parser {
    ocr: Option<Ocr>,
    ocr_error: Option<String>,
    pdf: Option<Pdfium>,
}
impl Parser {
    pub fn new() -> Self {
        Self {
            ocr: None,
            ocr_error: None,
            pdf: None,
        }
    }
    fn ocr(&mut self, img: DynamicImage, a: &mut Analysis, req: &Request) {
        if !req.settings.deep {
            a.issues
                .push("Immagine o pagina raster: OCR non eseguito in modalita rapida".into());
            return;
        }
        if self.ocr.is_none() && self.ocr_error.is_none() {
            let init = std::time::Instant::now();
            match unsafe { Ocr::load(req.settings.cpu_only) } {
                Ok(o) => self.ocr = Some(o),
                Err(e) => self.ocr_error = Some(e),
            }
            a.ocr_init_ms += init.elapsed().as_secs_f64() * 1000.;
        }
        let Some(ocr) = &self.ocr else {
            a.issues.push(self.ocr_error.clone().unwrap_or_default());
            return;
        };
        a.backend = ocr.backend.clone();
        let start = std::time::Instant::now();
        let mut current = img;
        for turn in 0..4 {
            match ocr.recognize(&current) {
                Ok(s) => {
                    let reasons = rules::detect(&s);
                    let found = !reasons.is_empty();
                    a.reasons.extend(reasons);
                    if found {
                        break;
                    }
                }
                Err(e) => {
                    a.issues.push(e);
                    break;
                }
            }
            if turn < 3 {
                current = current.rotate90();
            }
        }
        a.pages_ocr += 1;
        a.ocr_seconds += start.elapsed().as_secs_f64();
    }
    fn image_bytes(&mut self, bytes: &[u8], a: &mut Analysis, req: &Request) {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .unwrap();
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        limits.max_image_width = Some(7000);
        limits.max_image_height = Some(7000);
        reader.limits(limits);
        match reader.decode() {
            Ok(img) => self.ocr(img, a, req),
            Err(_) => a
                .issues
                .push("Immagine corrotta o limite 7000px / 128 MiB decodificati".into()),
        }
    }
    fn pdf(&mut self, bytes: &[u8], a: &mut Analysis, req: &Request) -> Result<(), String> {
        if self.pdf.is_none() {
            let bindings = Pdfium::bind_to_library(runtime().join("bin/pdfium.dll"))
                .map_err(|_| "PDFium non disponibile")?;
            self.pdf = Some(Pdfium::new(bindings));
        }
        // Separate PDFium instance ownership from OCR mutable access.
        let pdf = self.pdf.take().unwrap();
        let result = (|| {
            let doc = pdf
                .load_pdf_from_byte_slice(bytes, None)
                .map_err(|_| "PDF cifrato, corrotto o non leggibile")?;
            if doc.pages().len() as usize > req.settings.max_pages {
                a.issues.push("PDF: limite pagine raggiunto".into());
            }
            if !doc.attachments().is_empty() {
                a.issues.push("PDF: allegati incorporati non aperti".into());
            }
            for page in doc.pages().iter().take(req.settings.max_pages) {
                match page.text() {
                    Ok(t) => a.reasons.extend(rules::detect(&t.all())),
                    Err(_) => a.issues.push("PDF: estrazione testo fallita".into()),
                }
                if req.settings.deep {
                    let config = PdfRenderConfig::new()
                        .set_target_width(1700)
                        .set_maximum_height(2400)
                        .render_form_data(true);
                    match page.render_with_config(&config) {
                        Ok(bitmap) => self.ocr(bitmap.as_image(), a, req),
                        Err(_) => a.issues.push("PDF: rasterizzazione fallita".into()),
                    }
                } else {
                    a.issues
                        .push("PDF rapido: contenuto raster non esaminato".into());
                }
            }
            Ok(())
        })();
        self.pdf = Some(pdf);
        result
    }
    fn office(&mut self, bytes: &[u8], a: &mut Analysis, req: &Request) -> Result<(), String> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|_| "Archivio Office corrotto")?;
        if zip.len() > 3000 {
            return Err("Office: limite 3000 parti".into());
        }
        let mut used = 0u64;
        let mut images = 0;
        let mut text = String::new();
        let mut office = false;
        for i in 0..zip.len() {
            let mut f = zip
                .by_index(i)
                .map_err(|_| "Office: parte cifrata o illeggibile")?;
            let name = f.name().to_owned();
            if name.starts_with("word/")
                || name.starts_with("xl/")
                || name.starts_with("ppt/")
                || name == "content.xml"
            {
                office = true;
            }
            if f.is_dir() {
                continue;
            }
            let xml = ((name.starts_with("word/")
                || name.starts_with("xl/")
                || name.starts_with("ppt/")
                || name == "content.xml")
                && name.ends_with(".xml"))
                || name.ends_with(".rels");
            let media = name.contains("/media/") || name.starts_with("Pictures/");
            if name.contains("embeddings/") || name.ends_with("vbaProject.bin") {
                a.issues
                    .push("Office: oggetti OLE o macro non analizzati".into());
            }
            if !xml && !media {
                continue;
            }
            used = used.saturating_add(f.size());
            if used > 64 * 1024 * 1024 || f.size() > 16 * 1024 * 1024 {
                return Err("Office: limite decompressione 64 MiB / parte 16 MiB".into());
            }
            let mut bytes = Vec::new();
            f.by_ref()
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Office: decompressione fallita")?;
            if bytes.len() > 16 * 1024 * 1024 {
                return Err("Office: limite parte superato".into());
            }
            if xml {
                if name.ends_with(".rels") && bytes.windows(8).any(|s| s == b"External") {
                    a.issues.push("Office: risorse esterne non aperte".into());
                }
                let mut reader = quick_xml::Reader::from_reader(bytes.as_slice());
                loop {
                    match reader.read_event() {
                        Ok(quick_xml::events::Event::Text(t)) => {
                            text.push_str(&t.decode().map_err(|_| "Office: testo XML non valido")?);
                        }
                        Ok(quick_xml::events::Event::GeneralRef(r)) => {
                            let name = r
                                .decode()
                                .map_err(|_| "Office: riferimento XML non valido")?;
                            let character = match name.as_ref() {
                                "apos" => Some('\''),
                                "quot" => Some('"'),
                                "amp" => Some('&'),
                                "lt" => Some('<'),
                                "gt" => Some('>'),
                                s if s.starts_with("#x") => u32::from_str_radix(&s[2..], 16)
                                    .ok()
                                    .and_then(char::from_u32),
                                s if s.starts_with('#') => {
                                    s[1..].parse::<u32>().ok().and_then(char::from_u32)
                                }
                                _ => None,
                            };
                            text.push(character.ok_or("Office: riferimento XML non supportato")?);
                        }
                        Ok(quick_xml::events::Event::End(e)) => {
                            let local = e.local_name();
                            if [b"p".as_slice(), b"si", b"row", b"c"].contains(&local.as_ref()) {
                                text.push('\n');
                            }
                        }
                        Ok(quick_xml::events::Event::DocType(_)) => {
                            return Err("Office: DTD non ammessa".into());
                        }
                        Ok(quick_xml::events::Event::Eof) => break,
                        Err(_) => return Err("Office: XML corrotto".into()),
                        _ => {}
                    }
                    if text.len() > 4_000_000 {
                        return Err("Office: limite testo superato".into());
                    }
                }
                text.push('\n');
            } else if media {
                images += 1;
                if images <= req.settings.max_pages {
                    self.image_bytes(&bytes, a, req);
                } else {
                    a.issues.push("Office: limite immagini raggiunto".into());
                }
            }
        }
        if !office {
            return Err("Archivio generico non aperto come documento Office".into());
        }
        a.reasons.extend(rules::detect(&text));
        Ok(())
    }
    pub fn analyze(&mut self, req: &Request) -> Analysis {
        let mut a = Analysis {
            backend: "CPU / estrazione testuale".into(),
            ..Default::default()
        };
        let result = (|| {
            let mut f = OpenOptions::new()
                .read(true)
                .custom_flags(0x00200000)
                .open(&req.path)
                .map_err(|_| "File non leggibile: permessi o blocco")?;
            let before = f.metadata().map_err(|_| "Metadati non leggibili")?;
            if before.file_attributes() & (0x400 | 0x1000 | 0x400000 | 0x4000) != 0 {
                return Err("File reparse, offline, cloud recall o EFS non aperto".into());
            }
            if before.len() > req.settings.max_mb * 1024 * 1024 {
                return Err("Limite dimensione file".into());
            }
            let mut bytes = Vec::new();
            Read::by_ref(&mut f)
                .take(512)
                .read_to_end(&mut bytes)
                .map_err(|_| "Firma non leggibile")?;
            let head = &bytes[..bytes.len().min(512)];
            let ext = req
                .path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let is_pdf = head.starts_with(b"%PDF-");
            let is_zip = head.starts_with(b"PK\x03\x04");
            let img = image::guess_format(head).ok();
            let text_ext = ["txt", "csv", "tsv", "log", "md", "rtf"].contains(&ext.as_str());
            if !is_pdf && !is_zip && img.is_none() && !text_ext {
                return Err("Formato non supportato o archivio non aperto".into());
            }
            let cap = if text_ext && !is_pdf && !is_zip && img.is_none() {
                4_000_000
            } else {
                req.settings.max_mb * 1024 * 1024
            };
            Read::by_ref(&mut f)
                .take(cap.saturating_sub(bytes.len() as u64) + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Contenuto non leggibile")?;
            if bytes.len() as u64 > cap {
                return Err("Limite contenuto durante lettura".into());
            }
            if is_pdf {
                a.supported = true;
                a.format = "PDF".into();
                self.pdf(&bytes, &mut a, req)?;
            } else if is_zip {
                a.supported = true;
                a.format = "Office XML / ZIP".into();
                self.office(&bytes, &mut a, req)?;
            } else if let Some(image_format) = img {
                a.supported = true;
                a.format = format!("{image_format:?}");
                self.image_bytes(&bytes, &mut a, req);
                if matches!(
                    img,
                    Some(image::ImageFormat::Tiff | image::ImageFormat::Gif)
                ) {
                    a.issues
                        .push("TIFF/GIF: solo prima pagina o fotogramma".into());
                }
            } else if text_ext {
                a.supported = true;
                a.format = "Testo UTF-8".into();
                if bytes.len() > 4_000_000 {
                    return Err("Limite testo 4 MB".into());
                }
                match String::from_utf8(bytes) {
                    Ok(s) => a.reasons.extend(rules::detect(&s)),
                    Err(_) => return Err("Codifica testo non UTF-8".into()),
                }
                if ext == "rtf" {
                    a.issues
                        .push("RTF: solo testo grezzo, oggetti non analizzati".into());
                }
            } else {
                return Err("Formato non supportato o archivio non aperto".into());
            }
            let after = f
                .metadata()
                .map_err(|_| "File non piu leggibile durante il controllo")?;
            if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
                a.issues.push("File cambiato durante il controllo".into());
            }
            Ok(())
        })();
        if let Err(e) = result {
            a.issues.push(e);
        }
        a.reasons.sort();
        a.reasons.dedup();
        a.issues.sort();
        a.issues.dedup();
        a
    }
}

pub fn worker() {
    let mut parser = Parser::new();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.len() > 100_000 {
            break;
        }
        let Ok(req) = serde_json::from_str::<Request>(&line) else {
            break;
        };
        let a = parser.analyze(&req);
        if serde_json::to_writer(&mut out, &a).is_err()
            || writeln!(out).is_err()
            || out.flush().is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn office_entities_and_external_parts_are_not_silent() {
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("word/document.xml", options).unwrap();
        archive.write_all(b"<document><p>CARTA D&apos;IDENTITA</p><p>COGNOME ESEMPIO NOME PROVA NASCITA 01/01/2000 SCADENZA 01/01/2030 AA00000AA</p></document>").unwrap();
        archive
            .start_file("word/_rels/document.xml.rels", options)
            .unwrap();
        archive.write_all(b"<Relationships><Relationship TargetMode=\"External\" Target=\"https://example.invalid\" /></Relationships>").unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        let req = Request {
            path: "synthetic.docx".into(),
            settings: Default::default(),
        };
        let mut result = Analysis::default();
        Parser::new().office(&bytes, &mut result, &req).unwrap();
        assert_eq!(result.reasons.len(), 1);
        assert!(result.reasons[0].starts_with("Carta d'identita:"));
        assert!(result.issues.iter().any(|s| s.contains("risorse esterne")));
    }
}
