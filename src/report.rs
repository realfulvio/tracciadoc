use crate::model::{RULES, Run};
use pdfium_render::prelude::*;
use std::path::Path;

pub fn paragraphs(run: &Run, paths: bool, demo: bool) -> Vec<(String, f32)> {
    let mut lines = vec![];
    let mut add = |s: String, size: f32| {
        lines.push((s, size));
    };
    if demo {
        add("DIMOSTRATIVO - SOLO DOCUMENTI SINTETICI".into(), 12.);
    }
    add("Verbale di verifica dei documenti".into(), 19.);
    add("presenti sulla postazione".into(), 19.);
    add(
        format!(
            "TracciaDoc {} | regole {} | ID {}",
            env!("CARGO_PKG_VERSION"),
            RULES,
            run.id
        ),
        9.,
    );
    add(String::new(), 6.);
    if !run.settings.deep {
        add(
            "CONTROLLO RAPIDO PRELIMINARE - OCR NON ESEGUITO".into(),
            12.,
        );
    }
    add(run.outcome(), 12.);
    for line in run.finding_summary() {
        add(line, 10.);
    }
    add(
        format!("Postazione: {} | Operatore: {}", run.computer, run.operator),
        10.,
    );
    add(format!("Riferimento: {}", run.reference), 10.);
    add(
        format!(
            "Inizio: {} | Fine: {}",
            run.start,
            if run.end.is_empty() {
                "non conclusa"
            } else {
                &run.end
            }
        ),
        9.,
    );
    add(
        format!(
            "Date ISO 8601 con offset UTC esplicito. Durata: {:.2} secondi.",
            run.elapsed_secs
        ),
        9.,
    );
    add("AMBITO SELEZIONATO".into(), 12.);
    for path in &run.roots {
        add(path.to_string_lossy().into_owned(), 10.);
    }
    add("Altre unita e cartelle esterne ai percorsi selezionati non controllate. Collegamenti, junction e volumi montati discendenti non attraversati: selezionarli separatamente. Nessuna esclusione automatica di AppData, cestino, temporanei o profili; valgono i permessi dell'operatore.".into(),9.);
    add("COPERTURA E MOTORI".into(), 12.);
    add(
        format!(
            "Modalita: {}. Backend richiesto: {}. Effettivo: {}.",
            if run.settings.deep {
                "Ricerca completa supportata"
            } else {
                "Rapida preliminare"
            },
            if run.settings.cpu_only {
                "Solo CPU"
            } else {
                "Automatico (solo CPU implementata)"
            },
            run.backend
        ),
        9.,
    );
    add(
        format!(
            "PDFium 158.0.8125.0; OCR Tesseract CPU ita+eng se effettivamente usato. Pagine/immagini OCR: {}. Avvio OCR differito; nessun modello o provider scaricato. GPU/NPU non implementate; nessun fallback GPU dichiarato.",
            run.pages_ocr
        ),
        9.,
    );
    add("Formati: PDF (testo e raster di tutte le pagine entro limite), PNG, JPEG, BMP, WebP; GIF/TIFF solo prima immagine (limite segnalato). DOCX/XLSX/PPTX e ODT/ODS/ODP: XML e immagini raster incorporate. Testo UTF-8 TXT/CSV/TSV/LOG/MD. RTF solo testo grezzo. Ricerca completa: anche i PDF con testo digitale sono sottoposti a OCR.".into(),9.);
    add(
        format!(
            "Limiti: {} MiB/file; {} pagine o immagini/file; {} secondi/file; {} MiB memoria privata worker; {} file. Un parser persistente, un thread OCR. Raster PDF max 1700x2400, immagini max 7000 px/lato e 128 MiB. Office: 3000 parti, 64 MiB espansi, 16 MiB/parte. Nessuna cache; duplicati rianalizzati, percorsi conservati.",
            run.settings.max_mb,
            run.settings.max_pages,
            run.settings.timeout_secs,
            run.settings.memory_mb,
            run.settings.max_entries
        ),
        9.,
    );
    add("CONTEGGI".into(), 12.);
    add(
        format!(
            "File incontrati: {} | analizzati integralmente nelle capacita dichiarate: {} | parziali: {} | saltati/falliti: {} | file segnalati: {}",
            run.encountered,
            run.analyzed,
            run.partial_files,
            run.skipped,
            run.findings.len()
        ),
        10.,
    );
    add("Un file parziale o fallito non e contato come analizzato integralmente. Gli errori di enumerazione possono impedire di conoscere quanti file non siano stati raggiunti.".into(),9.);
    if !run.issues.is_empty() {
        add("ERRORI, ESCLUSIONI E LIMITI INCONTRATI".into(), 12.);
        for (issue, n) in &run.issues {
            add(format!("{n} - {issue}"), 9.);
        }
    }
    if !run.findings.is_empty() {
        add("SEGNALAZIONI DA VERIFICARE MANUALMENTE".into(), 12.);
        if paths {
            add(
                "Percorsi inclusi su scelta dell'operatore: possono contenere dati personali."
                    .into(),
                9.,
            );
        }
        for (i, f) in run.findings.iter().enumerate() {
            add(
                format!("{}. {} - {}", i + 1, f.format, f.reasons.join("; ")),
                9.,
            );
            add(format!("Controllo manuale: {}", f.review.label()), 9.);
            if paths {
                add(f.path.to_string_lossy().into_owned(), 9.);
            }
        }
    }
    add("CONCLUSIONE E SOTTOSCRIZIONE".into(), 12.);
    add("Riconoscimento euristico: possibili falsi positivi e negativi. Nessuna garanzia di trovare qualsiasi documento, ne certificazione dell'assenza assoluta. Nessuna approvazione Lepida; l'accettazione resta da verificare con il destinatario. Controllo in sola lettura, senza snapshot: non garantisce coerenza temporale dell'intero disco.".into(),9.);
    add("Non sono inclusi immagini, testo OCR o valori personali riconosciuti. Archivio locale limitato a eventi e conteggi, senza percorsi o contenuti. I processi isolati hanno limiti Windows Job ma non costituiscono una sandbox di sicurezza.".into(),9.);
    add(
        "Firma / riferimento alla sottoscrizione del responsabile: ________________________".into(),
        10.,
    );
    lines
}
fn wrap(
    s: &str,
    measure: impl Fn(&str) -> Result<f32, PdfiumError>,
) -> Result<Vec<String>, PdfiumError> {
    if s.is_empty() {
        return Ok(vec![String::new()]);
    }
    let mut out = vec![];
    let mut line = String::new();
    for word in s.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && measure(&candidate)? > 495. {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        if measure(word)? <= 495. {
            line.push_str(word);
            continue;
        }
        for c in word.chars() {
            if !line.is_empty() && measure(&format!("{line}{c}"))? > 495. {
                out.push(std::mem::take(&mut line));
            }
            line.push(c);
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    Ok(out)
}
pub fn write(run: &Run, path: &Path, paths: bool, demo: bool) -> Result<(), String> {
    let write_pdf = || -> Result<(), Box<dyn std::error::Error>> {
        let pdf = Pdfium::new(Pdfium::bind_to_library(
            crate::parser::runtime().join("bin/pdfium.dll"),
        )?);
        let mut doc = pdf.create_new_pdf()?;
        let font = doc.fonts_mut().load_true_type_from_bytes(
            include_bytes!("../assets/ui/fonts/TitilliumWeb-Regular.ttf"),
            true,
        )?;
        let heading = doc.fonts_mut().load_true_type_from_bytes(
            include_bytes!("../assets/ui/fonts/TitilliumWeb-SemiBold.ttf"),
            true,
        )?;
        let mut pages: Vec<Vec<(String, f32)>> = vec![vec![]];
        let mut used = 0.;
        for (text, size) in paragraphs(run, paths, demo) {
            let wrapped = wrap(&text, |s| {
                PdfPageTextObject::new(
                    &doc,
                    s,
                    if size >= 12. { heading } else { font },
                    PdfPoints::new(size),
                )?
                .width()
                .map(|w| w.value)
            })?;
            for line in &wrapped {
                let height = size + 3.5;
                if used + height > 716. {
                    pages.push(vec![]);
                    used = 0.;
                }
                let spacing = if size >= 12. { 5. } else { 0. };
                used += height + spacing;
                pages.last_mut().unwrap().push((line.clone(), size));
            }
        }
        let count = pages.len();
        for (index, lines) in pages.into_iter().enumerate() {
            let mut page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::a4())?;
            page.set_content_regeneration_strategy(PdfPageContentRegenerationStrategy::Manual);
            let mut y = 790.;
            for (text, size) in lines {
                if size >= 12. {
                    y -= 5.;
                }
                let mut obj = page.objects_mut().create_text_object(
                    PdfPoints::new(50.),
                    PdfPoints::new(y),
                    text,
                    if size >= 12. { heading } else { font },
                    PdfPoints::new(size),
                )?;
                if size >= 12. {
                    obj.set_fill_color(PdfColor::new(30, 72, 100, 255))?;
                }
                y -= size + 3.5;
            }
            page.objects_mut().create_text_object(
                PdfPoints::new(50.),
                PdfPoints::new(28.),
                format!(
                    "TracciaDoc | Verbale {} | Pagina {} / {}",
                    run.id,
                    index + 1,
                    count
                ),
                font,
                PdfPoints::new(8.),
            )?;
            page.regenerate_content()?;
        }
        // Atomic publication; refuse accidental overwrite rather than replacing an existing document.
        let folder = path.parent().unwrap_or_else(|| Path::new("."));
        let temp = tempfile::NamedTempFile::new_in(folder)?;
        let bytes = doc.save_to_bytes()?;
        std::fs::write(temp.path(), bytes)?;
        temp.persist_noclobber(path)?;
        Ok(())
    };
    write_pdf().map_err(|e| format!("Impossibile creare il PDF: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outcomes_keep_incompleteness_and_findings() {
        let mut r =
            crate::engine::new_run(vec![], Default::default(), "test".into(), "test".into());
        r.finished = true;
        assert!(r.outcome().contains("Nessun documento rilevato"));
        r.issue("accesso negato");
        assert!(r.outcome().contains("INCOMPLETA"));
        r.findings.push(crate::model::Finding {
            path: "sentinella-personale".into(),
            format: "test".into(),
            reasons: vec!["Motivo generico".into()],
            review: Default::default(),
        });
        assert!(r.outcome().contains("TROVATE"));
        assert!(r.outcome().contains("INCOMPLETA"));
        assert!(
            !paragraphs(&r, false, false)
                .iter()
                .any(|(s, _)| s.contains("sentinella-personale"))
        );
        assert!(
            paragraphs(&r, true, false)
                .iter()
                .any(|(s, _)| s.contains("sentinella-personale"))
        );
    }
}
