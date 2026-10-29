use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub const RULES: &str = "2026.10.28-1";
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub deep: bool,
    pub cpu_only: bool,
    pub max_mb: u64,
    pub max_pages: usize,
    pub timeout_secs: u64,
    pub max_entries: u64,
    pub memory_mb: u64,
    /// Parallel parser/OCR processes, each in its own Windows Job.
    #[serde(default = "one")]
    pub workers: usize,
    /// Skip OCR of images without text-like strokes (declared in the report).
    #[serde(default)]
    pub prefilter: bool,
}
fn one() -> usize {
    1
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            deep: true,
            cpu_only: false,
            max_mb: 64,
            max_pages: 40,
            timeout_secs: 120,
            max_entries: 1_000_000,
            memory_mb: 768,
            workers: crate::engine::default_workers(),
            prefilter: true,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub path: PathBuf,
    pub settings: Settings,
}
/// Tempi per fase (ms) per il benchmark; nessun contenuto o percorso.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Timing {
    pub files: u64,
    /// Round-trip coordinatore-worker per file (include tutto il lavoro del worker).
    pub roundtrip_ms: f64,
    /// Lavoro nel worker dalla richiesta alla risposta.
    pub worker_ms: f64,
    pub read_ms: f64,
    pub decode_ms: f64,
    pub render_ms: f64,
    pub ocr_ms: f64,
    pub ocr_calls: u64,
    pub ocr_images: u64,
    /// Immagini/pagine in cui nessuna rotazione ha prodotto testo alfanumerico utile.
    pub ocr_blank_images: u64,
    pub ocr_blank_ms: f64,
    #[serde(default)]
    pub prefilter_ms: f64,
    #[serde(default)]
    pub prefilter_skipped: u64,
    /// Orientation detection time, included in ocr_ms.
    #[serde(default)]
    pub osd_ms: f64,
    #[serde(default)]
    pub osd_hits: u64,
    /// Images reduced to their text-like region before OCR.
    #[serde(default)]
    pub prefilter_cropped: u64,
}
impl Timing {
    pub fn add(&mut self, o: &Timing) {
        self.files += o.files;
        self.roundtrip_ms += o.roundtrip_ms;
        self.worker_ms += o.worker_ms;
        self.read_ms += o.read_ms;
        self.decode_ms += o.decode_ms;
        self.render_ms += o.render_ms;
        self.ocr_ms += o.ocr_ms;
        self.ocr_calls += o.ocr_calls;
        self.ocr_images += o.ocr_images;
        self.ocr_blank_images += o.ocr_blank_images;
        self.ocr_blank_ms += o.ocr_blank_ms;
        self.prefilter_ms += o.prefilter_ms;
        self.prefilter_skipped += o.prefilter_skipped;
        self.osd_ms += o.osd_ms;
        self.osd_hits += o.osd_hits;
        self.prefilter_cropped += o.prefilter_cropped;
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Analysis {
    pub supported: bool,
    pub format: String,
    pub reasons: Vec<String>,
    pub issues: Vec<String>,
    pub pages_ocr: u64,
    pub ocr_seconds: f64,
    pub ocr_init_ms: f64,
    pub backend: String,
    #[serde(default)]
    pub timing: Timing,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Finding {
    pub path: PathBuf,
    pub format: String,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub review: Review,
    /// Selezionato dall'operatore per l'eliminazione.
    #[serde(default)]
    pub selected: bool,
    /// Eliminazione definitiva eseguita dall'operatore (data ISO 8601), vuoto se non eliminato.
    #[serde(default)]
    pub deleted_at: String,
    /// Ultimo errore di eliminazione, vuoto se nessuno.
    #[serde(default)]
    pub delete_error: String,
    /// Dimensione e data di modifica al momento dell'analisi: l'eliminazione le riverifica.
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub modified: Option<std::time::SystemTime>,
}
impl Finding {
    pub fn deleted(&self) -> bool {
        !self.deleted_at.is_empty()
    }
}
/// Riepilogo della verifica iniziale, riportato nel verbale della ri-scansione finale.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Previous {
    pub id: String,
    pub start: String,
    pub end: String,
    pub findings: u64,
    pub deleted: u64,
    pub failed: u64,
    pub incomplete: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Review {
    #[default]
    Pending,
    Confirmed,
    Discarded,
}
impl Review {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Da verificare",
            Self::Confirmed => "Documento confermato dall'operatore",
            Self::Discarded => "Segnalazione scartata dall'operatore",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub computer: String,
    pub operator: String,
    pub reference: String,
    pub roots: Vec<PathBuf>,
    pub settings: Settings,
    pub start: String,
    pub end: String,
    pub elapsed_secs: f64,
    pub encountered: u64,
    pub analyzed: u64,
    pub partial_files: u64,
    pub skipped: u64,
    /// File ignorati per tipo (estensione o firma non di interesse): solo conteggiati, mai nel log.
    #[serde(default)]
    pub ignored_type: u64,
    pub pages_ocr: u64,
    pub ocr_seconds: f64,
    pub ocr_init_ms: f64,
    pub issues: BTreeMap<String, u64>,
    pub findings: Vec<Finding>,
    pub interrupted: bool,
    pub finished: bool,
    pub backend: String,
    pub peak_worker_bytes: u64,
    pub audit_error: bool,
    #[serde(default)]
    pub previous: Option<Previous>,
    #[serde(default)]
    pub timing: Timing,
    #[serde(default)]
    pub timing_by_format: BTreeMap<String, Timing>,
}
impl Run {
    pub fn remaining(&self) -> usize {
        self.findings.iter().filter(|f| !f.deleted()).count()
    }
    pub fn deleted_count(&self) -> usize {
        self.findings.iter().filter(|f| f.deleted()).count()
    }
    pub fn summary(&self) -> Previous {
        Previous {
            id: self.id.clone(),
            start: self.start.clone(),
            end: self.end.clone(),
            findings: self.findings.len() as u64,
            deleted: self.deleted_count() as u64,
            failed: self
                .findings
                .iter()
                .filter(|f| !f.deleted() && !f.delete_error.is_empty())
                .count() as u64,
            incomplete: self.incomplete(),
        }
    }
    pub fn incomplete(&self) -> bool {
        !self.finished || self.interrupted || !self.issues.is_empty()
    }
    pub fn outcome(&self) -> String {
        let result = if self.findings.is_empty() {
            if self.incomplete() {
                "Nessuna segnalazione; copertura incompleta"
            } else if self.previous.is_some() {
                "Verifica finale: nessun documento rilevato nell'ambito esaminato dopo l'eliminazione"
            } else {
                "Nessun documento rilevato nell'ambito esaminato"
            }
        } else if self.remaining() == 0 {
            "Segnalazioni eliminate dall'operatore: ESEGUIRE LA RI-SCANSIONE DI VERIFICA, esito non ancora accertato"
        } else {
            "TROVATE possibili copie di documenti d'identita, da verificare"
        };
        format!(
            "{}{}",
            result,
            if self.interrupted {
                " - VERIFICA INTERROTTA"
            } else if self.incomplete() {
                " - VERIFICA INCOMPLETA"
            } else {
                ""
            }
        )
    }
    pub fn issue(&mut self, code: impl Into<String>) {
        *self.issues.entry(code.into()).or_default() += 1;
    }
    pub fn finding_summary(&self) -> Vec<String> {
        let mut formats: BTreeMap<&str, u64> = BTreeMap::new();
        let mut classes: BTreeMap<&str, u64> = BTreeMap::new();
        // Solo le segnalazioni non ancora eliminate: dopo l'eliminazione i conteggi devono coincidere con la lista.
        let live: Vec<&Finding> = self.findings.iter().filter(|f| !f.deleted()).collect();
        for file in live.iter().copied() {
            let kind = match file.format.to_ascii_lowercase().as_str() {
                "pdf" => "PDF",
                "png" | "jpeg" | "bmp" | "gif" | "tiff" | "webp" => "Immagini",
                "office xml / zip" => "Documenti Office",
                _ => "File di testo",
            };
            *formats.entry(kind).or_default() += 1;
            for (class, prefixes) in [
                ("Carte d'identita", &["Carta d'identita:", "MRZ TD1:"][..]),
                ("Passaporti", &["Passaporto:", "MRZ TD3:"][..]),
                ("Patenti", &["Patente:"][..]),
                ("Tessere sanitarie", &["Tessera sanitaria:"][..]),
            ] {
                if file
                    .reasons
                    .iter()
                    .any(|r| prefixes.iter().any(|p| r.starts_with(p)))
                {
                    *classes.entry(class).or_default() += 1;
                }
            }
        }
        let mut out = vec![format!(
            "File con possibili copie di documenti: {}",
            live.len()
        )];
        if !self.findings.is_empty() {
            out.push(format!(
                "Eliminazioni eseguite dall'operatore: {} | segnalazioni residue: {}",
                self.deleted_count(),
                self.remaining()
            ));
            if live.is_empty() {
                return out;
            }
            out.push(format!(
                "Tipi rilevati: {}",
                classes
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
            out.push(format!(
                "Dove sono state trovate: {}",
                formats
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
            out.push("Conteggi per tipo: file con indizi, non numero di documenti fisici. Un file puo contenere piu tipi.".into());
            out.push(format!(
                "Verifica manuale: {} confermati | {} scartati | {} ancora da verificare",
                live.iter()
                    .filter(|f| f.review == Review::Confirmed)
                    .count(),
                live.iter()
                    .filter(|f| f.review == Review::Discarded)
                    .count(),
                live.iter()
                    .filter(|f| f.review == Review::Pending)
                    .count()
            ));
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn summary_counts_follow_deletions() {
        let mut r = crate::engine::new_run(vec![], Default::default(), "t".into(), "t".into());
        for name in ["a.pdf", "b.pdf", "c.png"] {
            r.findings.push(Finding {
                path: name.into(),
                format: name.rsplit('.').next().unwrap().into(),
                reasons: vec!["Carta d'identita: indizio".into()],
                ..Default::default()
            });
        }
        assert_eq!(r.finding_summary()[0], "File con possibili copie di documenti: 3");
        r.findings[0].deleted_at = "2026-10-29T12:00:00+02:00".into();
        let text = r.finding_summary().join("\n");
        assert!(text.contains("possibili copie di documenti: 2"));
        assert!(text.contains("Eliminazioni eseguite dall'operatore: 1 | segnalazioni residue: 2"));
        assert!(text.contains("2 ancora da verificare"));
        for f in &mut r.findings {
            f.deleted_at = "x".into();
        }
        let text = r.finding_summary().join("\n");
        assert!(text.contains("possibili copie di documenti: 0"));
        assert!(text.contains("segnalazioni residue: 0"));
    }
}
