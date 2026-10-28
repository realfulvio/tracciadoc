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
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub path: PathBuf,
    pub settings: Settings,
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
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub path: PathBuf,
    pub format: String,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub review: Review,
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
}
impl Run {
    pub fn incomplete(&self) -> bool {
        !self.finished || self.interrupted || !self.issues.is_empty()
    }
    pub fn outcome(&self) -> String {
        let result = if self.findings.is_empty() {
            if self.incomplete() {
                "Nessuna segnalazione; copertura incompleta"
            } else {
                "Nessun documento rilevato nell'ambito esaminato"
            }
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
        for file in &self.findings {
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
            self.findings.len()
        )];
        if !self.findings.is_empty() {
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
                self.findings
                    .iter()
                    .filter(|f| f.review == Review::Confirmed)
                    .count(),
                self.findings
                    .iter()
                    .filter(|f| f.review == Review::Discarded)
                    .count(),
                self.findings
                    .iter()
                    .filter(|f| f.review == Review::Pending)
                    .count()
            ));
        }
        out
    }
}
