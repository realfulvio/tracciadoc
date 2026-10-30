//! Lettura dei vecchi file Word `.doc` (Compound File Binary + Word 97-2003), senza dipendenze.
//! Estrae il testo (tabella dei pezzi) e le immagini incorporate (record BLIP OfficeArt). Ogni
//! accesso e controllato: un file corrotto o malevolo produce un errore, mai un panico.
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

pub const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
/// Limite per singolo flusso decompresso in memoria.
const MAX_STREAM: usize = 64 * 1024 * 1024;
/// Limite del testo estratto (come per i documenti Office moderni).
const MAX_TEXT: usize = 4_000_000;
const FREE: u32 = 0xFFFF_FFFF;
const END: u32 = 0xFFFF_FFFE;

pub struct Doc {
    pub text: String,
    pub images: Vec<Vec<u8>>,
    pub issues: Vec<String>,
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o.checked_add(2)?)?.try_into().ok()?))
}
fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o.checked_add(4)?)?.try_into().ok()?))
}

struct Entry {
    name: String,
    kind: u8,
    left: u32,
    right: u32,
    child: u32,
    start: u32,
    size: u64,
}

struct Cfb<'a> {
    data: &'a [u8],
    sector: usize,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    mini_cutoff: u64,
    mini_stream: Vec<u8>,
    entries: Vec<Entry>,
}

impl<'a> Cfb<'a> {
    fn open(data: &'a [u8]) -> Result<Self, String> {
        const BAD: &str = "DOC: contenitore OLE corrotto o troncato";
        if data.len() < 512 || data[..8] != SIGNATURE {
            return Err(BAD.into());
        }
        let shift = u16_at(data, 0x1E).ok_or(BAD)?;
        let sector = match shift {
            9 => 512,
            12 => 4096,
            _ => return Err(BAD.into()),
        };
        if u16_at(data, 0x20) != Some(6) {
            return Err(BAD.into());
        }
        let mini_cutoff = u32_at(data, 0x38).ok_or(BAD)? as u64;
        let first_dir = u32_at(data, 0x30).ok_or(BAD)?;
        let first_mini = u32_at(data, 0x3C).ok_or(BAD)?;
        let n_mini = u32_at(data, 0x40).ok_or(BAD)? as usize;
        let mut difat_sector = u32_at(data, 0x44).ok_or(BAD)?;
        let n_difat = u32_at(data, 0x48).ok_or(BAD)? as usize;
        let total_sectors = (data.len() - 512).div_ceil(sector);
        let mut me = Cfb {
            data,
            sector,
            fat: Vec::new(),
            mini_fat: Vec::new(),
            mini_cutoff,
            mini_stream: Vec::new(),
            entries: Vec::new(),
        };
        // DIFAT: 109 voci nell'intestazione, poi una catena di settori.
        let mut fat_sectors: Vec<u32> = (0..109)
            .filter_map(|i| u32_at(data, 0x4C + i * 4))
            .filter(|s| *s < 0xFFFF_FFFA)
            .collect();
        let mut hops = 0;
        while difat_sector < 0xFFFF_FFFA && hops < n_difat.min(total_sectors) {
            let s = me.sector_bytes(difat_sector).ok_or(BAD)?;
            for i in 0..(sector / 4 - 1) {
                let v = u32_at(s, i * 4).ok_or(BAD)?;
                if v < 0xFFFF_FFFA {
                    fat_sectors.push(v);
                }
            }
            difat_sector = u32_at(s, sector - 4).ok_or(BAD)?;
            hops += 1;
        }
        if fat_sectors.len() > total_sectors {
            return Err(BAD.into());
        }
        for id in fat_sectors {
            let s = me.sector_bytes(id).ok_or(BAD)?;
            me.fat.extend(s.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)));
        }
        let dir = me.chain(first_dir, None)?;
        for chunk in dir.as_chunks::<128>().0.iter().take(1 << 20) {
            let name_len = (u16_at(chunk, 64).unwrap_or(0) as usize).min(64);
            let units: Vec<u16> = chunk[..name_len.saturating_sub(2)]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            me.entries.push(Entry {
                name: String::from_utf16_lossy(&units),
                kind: chunk[66],
                left: u32_at(chunk, 68).unwrap_or(FREE),
                right: u32_at(chunk, 72).unwrap_or(FREE),
                child: u32_at(chunk, 76).unwrap_or(FREE),
                start: u32_at(chunk, 116).unwrap_or(FREE),
                size: u32_at(chunk, 120).unwrap_or(0) as u64,
            });
        }
        let root = me.entries.first().ok_or(BAD)?;
        if root.kind != 5 {
            return Err(BAD.into());
        }
        let (root_start, root_size) = (root.start, root.size);
        if n_mini > 0 && first_mini < 0xFFFF_FFFA {
            let raw = me.chain(first_mini, None)?;
            me.mini_fat = raw.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect();
        }
        if root_start < 0xFFFF_FFFA && root_size > 0 {
            me.mini_stream = me.chain(root_start, Some(root_size))?;
        }
        Ok(me)
    }
    fn sector_bytes(&self, id: u32) -> Option<&'a [u8]> {
        let start = (id as usize).checked_add(1)?.checked_mul(self.sector)?;
        self.data.get(start..start.checked_add(self.sector)?)
    }
    /// Segue una catena FAT; `size` tronca il risultato (flussi) o la lascia intera (directory).
    fn chain(&self, mut id: u32, size: Option<u64>) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let mut steps = 0usize;
        while id < 0xFFFF_FFFA {
            steps += 1;
            if steps > self.fat.len() + 1 || out.len() > MAX_STREAM {
                return Err("DOC: catena di settori non valida o troppo grande".into());
            }
            out.extend_from_slice(
                self.sector_bytes(id).ok_or("DOC: contenitore OLE corrotto o troncato")?,
            );
            id = *self.fat.get(id as usize).unwrap_or(&END);
        }
        if let Some(size) = size {
            out.truncate(size as usize);
        }
        Ok(out)
    }
    fn mini_chain(&self, mut id: u32, size: u64) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let mut steps = 0usize;
        while id < 0xFFFF_FFFA {
            steps += 1;
            if steps > self.mini_fat.len() + 1 {
                return Err("DOC: catena mini non valida".into());
            }
            let start = id as usize * 64;
            out.extend_from_slice(
                self.mini_stream.get(start..start + 64).ok_or("DOC: contenitore OLE corrotto o troncato")?,
            );
            id = *self.mini_fat.get(id as usize).unwrap_or(&END);
        }
        out.truncate(size as usize);
        Ok(out)
    }
    /// Voci direttamente sotto la radice, visitando l'albero rosso-nero con protezione dai cicli.
    fn root_children(&self) -> Vec<&Entry> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![self.entries[0].child];
        while let Some(i) = stack.pop() {
            if i == FREE || !seen.insert(i) {
                continue;
            }
            if let Some(e) = self.entries.get(i as usize) {
                found.push(e);
                stack.push(e.left);
                stack.push(e.right);
            }
        }
        found
    }
    fn stream(&self, name: &str) -> Option<Result<Vec<u8>, String>> {
        let e = self.root_children().into_iter().find(|e| e.kind == 2 && e.name == name)?;
        if e.size > MAX_STREAM as u64 {
            return Some(Err("DOC: flusso troppo grande".into()));
        }
        Some(if e.size < self.mini_cutoff { self.mini_chain(e.start, e.size) } else { self.chain(e.start, Some(e.size)) })
    }
    fn has_storage(&self, name: &str) -> bool {
        self.root_children().into_iter().any(|e| e.kind == 1 && e.name == name)
    }
}

const CP1252_HIGH: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’',
    '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

fn push_char(text: &mut String, c: char) {
    match c {
        '\r' | '\u{7}' | '\u{b}' | '\u{c}' => text.push('\n'),
        '\t' | '\n' => text.push(c),
        // 0x13/0x14/0x15 delimitano i campi, 0x01 e 0x08 sono segnaposto di oggetti.
        c if (c as u32) < 0x20 => {}
        c => text.push(c),
    }
}
fn push_cp1252(text: &mut String, bytes: &[u8]) {
    for &b in bytes {
        push_char(text, if (0x80..0xA0).contains(&b) { CP1252_HIGH[(b - 0x80) as usize] } else { b as char });
    }
}
fn push_utf16(text: &mut String, bytes: &[u8]) {
    let units = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c));
    for c in char::decode_utf16(units) {
        push_char(text, c.unwrap_or('\u{fffd}'));
    }
}

/// Testo di un documento Word 97-2003 e successivi: tabella dei pezzi (Clx) nel flusso Table.
fn word_text(wd: &[u8], table: &[u8], issues: &mut Vec<String>) -> Result<String, String> {
    const BAD: &str = "DOC: struttura Word non valida";
    let csw = u16_at(wd, 32).ok_or(BAD)? as usize;
    let cslw_at = 34 + csw * 2;
    let cslw = u16_at(wd, cslw_at).ok_or(BAD)? as usize;
    let cb_at = cslw_at + 2 + cslw * 4;
    let pairs = u16_at(wd, cb_at).ok_or(BAD)? as usize;
    // fcClx / lcbClx e la coppia 33 di FibRgFcLcb97.
    if pairs < 34 {
        return Err(BAD.into());
    }
    let at = cb_at + 2 + 33 * 8;
    let fc = u32_at(wd, at).ok_or(BAD)? as usize;
    let lcb = u32_at(wd, at + 4).ok_or(BAD)? as usize;
    let clx = table.get(fc..fc.checked_add(lcb).ok_or(BAD)?).ok_or(BAD)?;
    let mut pos = 0;
    while clx.get(pos) == Some(&1) {
        pos += 3 + u16_at(clx, pos + 1).ok_or(BAD)? as usize;
    }
    if clx.get(pos) != Some(&2) {
        return Err(BAD.into());
    }
    let plc_len = u32_at(clx, pos + 1).ok_or(BAD)? as usize;
    let plc = clx.get(pos + 5..pos + 5 + plc_len).ok_or(BAD)?;
    if plc_len < 16 || !(plc_len - 4).is_multiple_of(12) {
        return Err(BAD.into());
    }
    let n = (plc_len - 4) / 12;
    let mut text = String::new();
    let mut truncated = false;
    for i in 0..n {
        let cp0 = u32_at(plc, i * 4).ok_or(BAD)? as usize;
        let cp1 = u32_at(plc, i * 4 + 4).ok_or(BAD)? as usize;
        let pcd = (n + 1) * 4 + i * 8;
        let fc = u32_at(plc, pcd + 2).ok_or(BAD)?;
        let compressed = fc & 0x4000_0000 != 0;
        let fc = (fc & 0x3FFF_FFFF) as usize;
        let len = cp1.saturating_sub(cp0);
        let (off, bytes) = if compressed { (fc / 2, len) } else { (fc, len.saturating_mul(2)) };
        let Some(raw) = off.checked_add(bytes).and_then(|end| wd.get(off..end)) else {
            truncated = true;
            continue;
        };
        if compressed {
            push_cp1252(&mut text, raw)
        } else {
            push_utf16(&mut text, raw)
        }
        if text.len() > MAX_TEXT {
            issues.push("DOC: limite testo superato".into());
            return Ok(text);
        }
    }
    if truncated {
        issues.push("DOC: parte del testo fuori dal file, non analizzata".into());
    }
    Ok(text)
}

/// Word 6/95: testo a 8 bit contiguo fra fcMin e fcMac.
fn word6_text(wd: &[u8]) -> Result<String, String> {
    const BAD: &str = "DOC: struttura Word non valida";
    let (min, mac) = (u32_at(wd, 0x18).ok_or(BAD)? as usize, u32_at(wd, 0x1C).ok_or(BAD)? as usize);
    let raw = wd.get(min..mac.min(min + MAX_TEXT)).ok_or(BAD)?;
    let mut text = String::new();
    push_cp1252(&mut text, raw);
    Ok(text)
}

/// DIB (BITMAPINFOHEADER senza intestazione di file) -> BMP leggibile dal decoder.
fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    let h = u32_at(dib, 0)? as usize;
    let palette = if h == 12 {
        let bits = u16_at(dib, 10)?;
        if bits <= 8 { (1usize << bits) * 3 } else { 0 }
    } else if (40..=124).contains(&h) {
        let bits = u16_at(dib, 14)?;
        let compression = u32_at(dib, 16)?;
        let used = u32_at(dib, 32)? as usize;
        let colors = if used > 0 { used } else if bits <= 8 { 1usize << bits } else { 0 };
        let masks = if h == 40 && compression == 3 { 12 } else if h == 40 && compression == 6 { 16 } else { 0 };
        colors.checked_mul(4)?.checked_add(masks)?
    } else {
        return None;
    };
    let offset = 14 + h + palette;
    let mut out = Vec::with_capacity(14 + dib.len());
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

/// Cerca i record BLIP OfficeArt (JPEG, PNG, TIFF, DIB) in un flusso. Le immagini inline stanno nel
/// flusso Data, quelle mobili nel flusso WordDocument (FBSE con foDelay): la scansione per
/// intestazione le trova in entrambi i casi senza dover interpretare i riferimenti.
fn scan_blips(buf: &[u8], images: &mut Vec<Vec<u8>>, seen: &mut HashSet<u64>, metafile: &mut bool) {
    let mut i = 0;
    while i + 8 <= buf.len() {
        if buf[i + 3] != 0xF0 || buf[i] & 0x0F != 0 {
            i += 1;
            continue;
        }
        let ty = u16::from_le_bytes([buf[i + 2], buf[i + 3]]);
        let inst = u16::from_le_bytes([buf[i], buf[i + 1]]) >> 4;
        let len = u32_at(buf, i + 4).unwrap_or(0) as usize;
        let valid_len = len >= 17 && len <= buf.len() - i - 8;
        if !(0xF018..=0xF117).contains(&ty) || !valid_len {
            i += 1;
            continue;
        }
        let body = &buf[i + 8..i + 8 + len];
        let payload = |uids: usize| body.get(16 * uids + 1..);
        let image: Option<Vec<u8>> = match (ty, inst) {
            (0xF01D | 0xF02A, 0x46A | 0x6E2) => payload(1).filter(|d| d.starts_with(&[0xFF, 0xD8])).map(<[u8]>::to_vec),
            (0xF01D | 0xF02A, 0x46B | 0x6E3) => payload(2).filter(|d| d.starts_with(&[0xFF, 0xD8])).map(<[u8]>::to_vec),
            (0xF01E, 0x6E0) => payload(1).filter(|d| d.starts_with(&[0x89, b'P', b'N', b'G'])).map(<[u8]>::to_vec),
            (0xF01E, 0x6E1) => payload(2).filter(|d| d.starts_with(&[0x89, b'P', b'N', b'G'])).map(<[u8]>::to_vec),
            (0xF029, 0x6E4) => payload(1).filter(|d| d.starts_with(b"II*\0") || d.starts_with(b"MM\0*")).map(<[u8]>::to_vec),
            (0xF029, 0x6E5) => payload(2).filter(|d| d.starts_with(b"II*\0") || d.starts_with(b"MM\0*")).map(<[u8]>::to_vec),
            (0xF01F, 0x7A8) => payload(1).and_then(dib_to_bmp),
            (0xF01F, 0x7A9) => payload(2).and_then(dib_to_bmp),
            (0xF01A, 0x3D4 | 0x3D5) | (0xF01B, 0x216 | 0x217) | (0xF01C, 0x542 | 0x543) => {
                *metafile = true;
                None
            }
            _ => None,
        };
        match image {
            Some(img) => {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                img.hash(&mut h);
                if seen.insert(h.finish()) {
                    images.push(img);
                }
                i += 8 + len;
            }
            None if (0xF01A..=0xF01C).contains(&ty) => i += 8 + len,
            None => i += 1,
        }
    }
}

pub fn read(data: &[u8]) -> Result<Doc, String> {
    let cfb = Cfb::open(data)?;
    let wd = cfb
        .stream("WordDocument")
        .ok_or("DOC: flusso WordDocument assente, non e un documento Word")??;
    let ident = u16_at(&wd, 0).ok_or("DOC: struttura Word non valida")?;
    if ident != 0xA5EC && ident != 0xA5DC {
        return Err("DOC: intestazione Word non riconosciuta".into());
    }
    let nfib = u16_at(&wd, 2).unwrap_or(0);
    let flags = u16_at(&wd, 0x0A).ok_or("DOC: struttura Word non valida")?;
    if flags & 0x0100 != 0 {
        return Err("DOC: documento Word cifrato, non analizzato".into());
    }
    let mut issues = Vec::new();
    let text = if nfib < 0x00C1 {
        word6_text(&wd)?
    } else {
        let name = if flags & 0x0200 != 0 { "1Table" } else { "0Table" };
        let table = cfb.stream(name).ok_or("DOC: flusso Table assente")??;
        word_text(&wd, &table, &mut issues)?
    };
    let mut images = Vec::new();
    let mut seen = HashSet::new();
    let mut metafile = false;
    scan_blips(&wd, &mut images, &mut seen, &mut metafile);
    if let Some(data_stream) = cfb.stream("Data") {
        scan_blips(&data_stream?, &mut images, &mut seen, &mut metafile);
    }
    if metafile {
        issues.push("DOC: immagini vettoriali (EMF/WMF/PICT) non analizzate".into());
    }
    if cfb.has_storage("ObjectPool") || cfb.has_storage("Macros") {
        issues.push("DOC: oggetti OLE o macro non analizzati".into());
    }
    Ok(Doc { text, images, issues })
}

#[cfg(test)]
mod tests {
    use super::*;
    const TESTO: &[u8] = include_bytes!("../fixtures/corpus/documento.doc");
    const IMMAGINE: &[u8] = include_bytes!("../fixtures/corpus/immagine.doc");

    #[test]
    fn legge_il_testo_di_un_doc_vero() {
        let d = read(TESTO).unwrap();
        assert!(d.text.contains("CARTA DI IDENTITA"), "{}", d.text);
        assert!(d.issues.is_empty(), "{:?}", d.issues);
    }
    #[test]
    fn estrae_le_immagini_incorporate() {
        let d = read(IMMAGINE).unwrap();
        assert!(d.text.contains("Nota ordinaria"), "{}", d.text);
        assert_eq!(d.images.len(), 1);
        assert!(d.images[0].starts_with(&[0x89, b'P', b'N', b'G']) || d.images[0].starts_with(&[0xFF, 0xD8]));
    }
    #[test]
    fn file_troncati_o_alterati_non_vanno_in_panico() {
        for src in [TESTO, IMMAGINE] {
            for cut in (0..src.len()).step_by(997) {
                let _ = read(&src[..cut]);
            }
            let mut seed = 0x1234_5678u32;
            for _ in 0..400 {
                let mut v = src.to_vec();
                for _ in 0..8 {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let at = (seed >> 8) as usize % v.len();
                    v[at] = (seed >> 24) as u8;
                }
                let _ = read(&v);
            }
        }
    }
    #[test]
    fn firma_senza_contenuto_e_corrotta() {
        let mut v = SIGNATURE.to_vec();
        v.extend_from_slice(b"fixture-not-a-document");
        assert!(read(&v).is_err());
    }
    #[test]
    fn cp1252_e_controlli() {
        let mut s = String::new();
        push_cp1252(&mut s, b"A\x96B\x0d\x07\x13\xe8");
        assert_eq!(s, "A\u{2013}B\n\n\u{e8}");
    }
    #[test]
    fn dib_diventa_bmp_decodificabile() {
        let mut dib = vec![0u8; 40];
        dib[0] = 40;
        dib[4] = 1;
        dib[8] = 1;
        dib[12] = 1;
        dib[14] = 24;
        dib.extend_from_slice(&[0, 0, 255, 0]);
        let bmp = dib_to_bmp(&dib).unwrap();
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(u32::from_le_bytes([bmp[10], bmp[11], bmp[12], bmp[13]]), 54);
    }
}
