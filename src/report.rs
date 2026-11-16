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
    add(
        if run.previous.is_some() {
            "Verbale di verifica finale dei documenti"
        } else {
            "Verbale di verifica dei documenti"
        }
        .into(),
        19.,
    );
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
    if let Some(p) = &run.previous {
        add("VERIFICA INIZIALE E ELIMINAZIONI".into(), 12.);
        add(
            format!(
                "Verifica iniziale ID {} ({} - {}): file segnalati {}, eliminati dall'operatore {}, non eliminati {}{}{}.",
                p.id,
                p.start,
                p.end,
                p.findings,
                p.deleted,
                p.findings.saturating_sub(p.deleted),
                if p.failed > 0 { format!(" (di cui {} per eliminazione non riuscita)", p.failed) } else { String::new() },
                if p.incomplete { ", copertura incompleta" } else { "" }
            ),
            10.,
        );
        add("La presente verifica e stata rieseguita dopo le eliminazioni sugli stessi percorsi selezionati.".into(), 9.);
    }
    if run.deleted_count() > 0 {
        add("Eliminazioni eseguite dopo questa verifica: il verbale NON documenta la situazione finale; rieseguire la verifica.".into(), 10.);
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
    if paths {
        for path in &run.roots {
            add(report_path(path), 10.);
        }
    } else {
        add(format!("{} aree selezionate; percorsi omessi su scelta dell'operatore.", run.roots.len()), 10.);
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
            "PDFium 158.0.8125.0; OCR Tesseract CPU italiano se effettivamente usato. Pagine/immagini OCR: {}. Avvio OCR differito; nessun modello o provider scaricato. GPU/NPU non implementate; nessun fallback GPU dichiarato.",
            run.pages_ocr
        ),
        9.,
    );
    add("Formati: PDF, PNG, JPEG, BMP, WebP, TIFF (tutte le pagine entro limite); GIF solo primo fotogramma (limite segnalato). DOCX/XLSX/PPTX (anche varianti con macro, modelli e presentazioni) e ODT/ODS/ODP: XML e immagini raster incorporate. Word DOC: testo e immagini raster supportate. RTF solo testo grezzo. I file di solo testo non vengono analizzati; alcune estensioni generiche, inclusa TXT, sono controllate per riconoscere documenti rinominati. PDF: testo digitale e OCR, salvo pagine verificate con soli oggetti di testo leggibili. Archivi, contenitori di posta e flussi alternativi NTFS presenti ma non letti rendono la verifica incompleta. Appena una pagina o immagine di un file rileva un documento, le successive dello stesso file non vengono lette: l'esito del file non puo cambiare.".into(),9.);
    add(
        format!(
            "Limiti: {} MiB/file; {} pagine o immagini/file; {} secondi per tentativo; {} MiB memoria privata per worker; {} file. {} worker parser/OCR in parallelo, ognuno processo separato con un thread OCR. Raster PDF a {} dpi (max 3600 px/lato), immagini max 10000 px/lato e 128 MiB decodificati, OCR a 3600 px/lato al massimo. Office: 3000 parti, 64 MiB espansi, 16 MiB/parte. Riuso esiti: un file con contenuto identico (SHA-256) a uno gia analizzato in questa sessione (analisi conclusa, non interrotta, file invariato durante la lettura), con stessa estensione, impostazioni e versione, riceve lo stesso esito verificato senza nuova lettura; file con esito riusato: {}. Ogni percorso e elencato e contato singolarmente. Nessuna cache su disco.",
            run.settings.max_mb,
            run.settings.max_pages,
            run.settings.timeout_secs,
            run.settings.memory_mb,
            run.settings.max_entries,
            run.settings.workers,
            crate::parser::pdf_dpi(),
            run.timing.reused
        ),
        9.,
    );
    add(format!("I timeout sono ritentati una volta, in sequenza dopo la fase parallela, con lo stesso limite per tentativo (massimo 10000 riprove). Tentativi ripetuti: {}; risposte ottenute in riprova: {}. Pagine PDF con soli oggetti di testo leggibili, senza immagini, grafica o annotazioni, esaminate senza OCR superfluo: {}.", run.timing.timeout_retries, run.timing.timeout_recovered, run.timing.pdf_text_only_pages), 9.);
    add("CONTEGGI".into(), 12.);
    add(
        format!(
            "File incontrati: {} | analizzati integralmente nelle capacita dichiarate: {} | parziali: {} | saltati/falliti: {} | ignorati per tipo (estensione o firma non di interesse): {} | file segnalati: {}",
            run.encountered,
            run.analyzed,
            run.partial_files,
            run.skipped,
            run.ignored_type,
            run.findings.len()
        ),
        10.,
    );
    add("Un file parziale o fallito non e contato come analizzato integralmente. Gli errori di enumerazione possono impedire di conoscere quanti file non siano stati raggiunti.".into(),9.);
    add(
        if run.settings.prefilter {
            format!(
                "Prefiltro immagini attivo: immagini o pagine scartate dal prefiltro, senza OCR: {}. Sono quelle prive di tratti a contrasto tipo testo in ogni zona; i file che le contengono sono contati come parziali.",
                run.timing.prefilter_skipped
            )
        } else {
            "Prefiltro immagini disattivato: OCR su tutte le immagini e pagine entro i limiti.".into()
        },
        9.,
    );
    add(
        if run.settings.photo_id {
            format!(
                "Fototessere: rilevatore di volti offline attivo; immagini controllate: {}, segnalate come fototessera: {}. Sono indizi da verificare: un volto frontale unico, grande, su sfondo uniforme. Una foto piccola dentro una pagina o un volto di profilo non e segnalato.",
                run.timing.photo_checks, run.timing.photo_found
            )
        } else {
            "Fototessere: ricerca disattivata dall'operatore.".into()
        },
        9.,
    );
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
            if f.deleted() {
                add(format!("Eliminato definitivamente dall'operatore: {}", f.deleted_at), 9.);
            } else if !f.delete_error.is_empty() {
                add(format!("Eliminazione non riuscita: {}", f.delete_error), 9.);
            }
            if paths {
                add(report_path(&f.path), 9.);
            }
        }
    }
    if paths && !run.coverage_gaps.is_empty() {
        add("DETTAGLI DEI FILE O DELLE AREE NON COMPLETAMENTE ESAMINATI".into(), 12.);
        for gap in &run.coverage_gaps {
            add(gap.reason.clone(), 9.);
            add(report_path(&gap.path), 9.);
        }
    }
    add("CONCLUSIONE E SOTTOSCRIZIONE".into(), 12.);
    add("Riconoscimento euristico: possibili falsi positivi e negativi. Nessuna garanzia di trovare qualsiasi documento, ne certificazione dell'assenza assoluta. Nessuna approvazione Lepida; l'accettazione resta da verificare con il destinatario. Controllo in sola lettura, senza snapshot: non garantisce coerenza temporale dell'intero disco.".into(),9.);
    add("L'eliminazione rimuove il file senza passare dal Cestino; non sovrascrive lo spazio liberato e non garantisce irrecuperabilita. Backup, copie VSS, cache e spazio non allocato non sono verificati da questa scansione.".into(),9.);
    add("Non sono inclusi immagini, testo OCR o valori personali riconosciuti. L'audit conserva eventi e conteggi senza percorsi o contenuti. La diagnostica e l'esito locale delle scansioni pianificate possono contenere percorsi: conservarli in modo riservato. I processi isolati hanno limiti Windows Job ma non costituiscono una sandbox di sicurezza.".into(),9.);
    add(
        "Firma / riferimento alla sottoscrizione del responsabile: ________________________".into(),
        10.,
    );
    lines
}
/// The bundled font cannot render every Unicode script. An explicit, reversible escaped
/// representation preserves every scalar value instead of silently dropping path characters.
fn report_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if text.is_ascii() && !text.chars().any(char::is_control)
        && !text.starts_with(char::is_whitespace) && !text.ends_with(char::is_whitespace)
        && !text.contains("  ") {
        text.into_owned()
    } else {
        let escaped: String = text.chars().map(|c| {
            if c.is_whitespace() { format!("\\u{{{:x}}}", c as u32) }
            else { c.escape_default().to_string() }
        }).collect();
        format!("Percorso con escape Unicode: {escaped}")
    }
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
            if index == 0
                && let Some(logo) = org_logo()
            {
                {
                    // Logo on the white page: alpha flattened on white, PDF images have no transparency.
                    let mut flat = image::RgbImage::from_pixel(logo.width(), logo.height(), image::Rgb([255, 255, 255]));
                    for (x, y, p) in logo.enumerate_pixels() {
                        let a = p[3] as u32;
                        let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
                        flat.put_pixel(x, y, image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])]));
                    }
                    page.objects_mut().create_image_object(
                        PdfPoints::new(470.),
                        PdfPoints::new(715.),
                        &image::DynamicImage::ImageRgb8(flat),
                        Some(PdfPoints::new(80.)),
                        Some(PdfPoints::new(80.)),
                    )?;
                }
            }
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
    if path.exists() {
        // Anche se la finestra di salvataggio ha chiesto conferma: un verbale non si sovrascrive.
        return Err("Esiste gia un file con questo nome: il verbale non viene sovrascritto, scegli un altro nome".into());
    }
    write_pdf().map_err(|e| format!("Impossibile creare il PDF: {e}"))
}

/// Logo facoltativo dell'ente: `logo-ente.png` accanto all'EXE. Non fa parte del repository ne del pacchetto.
pub fn org_logo() -> Option<image::RgbaImage> {
    let path = std::env::current_exe().ok()?.parent()?.join("logo-ente.png");
    if std::fs::metadata(&path).ok()?.len() > 2 << 20 {
        return None;
    }
    let image = image::open(path).ok()?.to_rgba8();
    (image.width() <= 2048 && image.height() <= 2048).then_some(image)
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
            ..Default::default()
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
    #[test]
    fn unicode_paths_remain_identifiable_and_omitted_paths_stay_private() {
        let path = std::path::PathBuf::from("C:\\synthetic\\Ж中😀.pdf");
        let rendered = report_path(&path);
        assert!(rendered.is_ascii());
        assert!(rendered.contains("\\u{416}\\u{4e2d}\\u{1f600}"));
        assert!(report_path(Path::new("C:\\synthetic\\trailing.jpg ")).ends_with("\\u{20}"));
        let mut r = crate::engine::new_run(vec![path.clone()], Default::default(), "test".into(), "test".into());
        r.finished = true;
        r.issue_at(&path, crate::streams::NOT_ANALYZED);
        assert!(r.incomplete());
        assert!(paragraphs(&r, true, false).iter().any(|(s, _)| s == &rendered));
        assert!(!paragraphs(&r, false, false).iter().any(|(s, _)| s.contains("synthetic")));
        assert!(!serde_json::to_string(&r.issues).unwrap().contains("synthetic"));
    }
    #[test]
    fn reopening_scheduled_review_preserves_coverage_details_and_reads_old_runs() {
        let path = std::path::PathBuf::from("C:\\synthetic\\archive.zip");
        let mut r = crate::engine::new_run(vec![path.clone()], Default::default(), "test".into(), "test".into());
        r.finished = true;
        r.issue_at(&path, crate::parser::CONTAINER_NOT_READ);
        let mut stored = serde_json::to_value(&r).unwrap();
        let reopened: Run = serde_json::from_value(stored.clone()).unwrap();
        assert_eq!(reopened.coverage_gaps.len(), 1);
        assert!(paragraphs(&reopened, true, false).iter().any(|(s, _)| s == &report_path(&path)));
        // Existing scheduled reviews predate the field and must still open.
        stored.as_object_mut().unwrap().remove("coverage_gaps");
        let old: Run = serde_json::from_value(stored).unwrap();
        assert!(old.coverage_gaps.is_empty());
        assert!(old.incomplete());
    }
}
