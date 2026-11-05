//! Salvataggio automatico dei verbali PDF in una cartella condivisa configurabile.
//!
//! Il percorso non e nel codice. Ordine di ricerca: argomento `--report-dir`,
//! `tracciadoc.toml` accanto all'EXE (`report_dir = "..."`), poi la chiave di registro
//! `HKLM\Software\TracciaDoc` valore `ReportDir`. Funziona senza GUI e senza unita mappate
//! (percorso UNC), quindi anche da attivita pianificate. Se la condivisa non e raggiungibile
//! il verbale resta nella cartella locale e viene riprovato in seguito (`riprova_sospesi`).
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Esito {
    /// Copia verificata (dimensione e SHA-256) nella cartella condivisa.
    Condivisa(PathBuf),
    /// Condivisa non raggiungibile: verbale solo in locale, marcato come in sospeso.
    SoloLocale(String),
    /// Nessuna cartella condivisa configurata.
    NonConfigurata,
}

/// Estrae `report_dir` da un testo in stile TOML (solo stringhe semplici).
pub fn parse_config(text: &str) -> Option<PathBuf> {
    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "report_dir" {
            continue;
        }
        let value = value.trim();
        let path = if let Some(v) = value.strip_prefix('\'') {
            v.split('\'').next()?.to_string()
        } else {
            let v = value.strip_prefix('"')?;
            let end = v.rfind('"')?;
            v[..end].replace("\\\\", "\\")
        };
        if !path.trim().is_empty() {
            return Some(PathBuf::from(path.trim()));
        }
    }
    None
}

#[cfg(windows)]
fn registry_dir() -> Option<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    let key: Vec<u16> = "Software\\TracciaDoc\0".encode_utf16().collect();
    let name: Vec<u16> = "ReportDir\0".encode_utf16().collect();
    let mut buf = [0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if rc != 0 {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buf.len());
    let s = OsString::from_wide(&buf[..len]);
    (!s.is_empty()).then(|| PathBuf::from(s))
}
#[cfg(not(windows))]
fn registry_dir() -> Option<PathBuf> {
    None
}

/// Cartella condivisa configurata, se presente.
pub fn report_dir(cli: Option<&str>) -> Option<PathBuf> {
    if let Some(c) = cli.filter(|c| !c.trim().is_empty()) {
        return Some(PathBuf::from(c));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && let Ok(text) = std::fs::read_to_string(dir.join("tracciadoc.toml"))
        && let Some(p) = parse_config(&text)
    {
        return Some(p);
    }
    registry_dir()
}

/// Cartella locale dei verbali: Documenti dell'utente, oppure ProgramData (es. account SYSTEM).
pub fn local_dir() -> PathBuf {
    let base = match std::env::var_os("USERPROFILE") {
        Some(p) if !p.to_string_lossy().to_lowercase().contains("systemprofile") => {
            PathBuf::from(p).join("Documents")
        }
        _ => PathBuf::from(
            std::env::var_os("ProgramData").unwrap_or_else(|| "C:\\ProgramData".into()),
        ),
    };
    base.join("TracciaDoc").join("Verbali")
}

/// `Verbale-TracciaDoc-<PC>-<AAAA-MM-GG_HHMMSS>.pdf`
pub fn file_name(computer: &str, now: chrono::NaiveDateTime) -> String {
    let pc: String = computer
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!(
        "Verbale-TracciaDoc-{pc}-{}.pdf",
        now.format("%Y-%m-%d_%H%M%S")
    )
}

fn sha(path: &Path) -> std::io::Result<(u64, Vec<u8>)> {
    let bytes = std::fs::read(path)?;
    Ok((bytes.len() as u64, Sha256::digest(&bytes).to_vec()))
}

fn pending_marker(pdf: &Path) -> PathBuf {
    let mut s = pdf.as_os_str().to_os_string();
    s.push(".sospeso");
    PathBuf::from(s)
}

/// Copia `src` in `dir` (3 tentativi), pubblica in modo atomico senza sovrascrivere e verifica SHA-256.
fn copia_verificata(src: &Path, dir: &Path, pause: Duration) -> Result<PathBuf, String> {
    let name = src.file_name().ok_or("Nome file non valido")?;
    let mut last = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(pause * attempt);
        }
        let result = (|| -> Result<PathBuf, String> {
            std::fs::create_dir_all(dir).map_err(|e| format!("cartella non raggiungibile: {e}"))?;
            let mut dest = dir.join(name);
            if dest.exists() {
                // Stesso nome gia presente: se identico e gia consegnato, altrimenti non si sovrascrive.
                if sha(&dest).map_err(|e| e.to_string())? == sha(src).map_err(|e| e.to_string())? {
                    return Ok(dest);
                }
                let stem = dest
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                dest = dir.join(format!("{stem}-bis.pdf"));
            }
            let temp = tempfile::NamedTempFile::new_in(dir)
                .map_err(|e| format!("scrittura non riuscita: {e}"))?;
            std::fs::copy(src, temp.path()).map_err(|e| format!("copia non riuscita: {e}"))?;
            temp.persist_noclobber(&dest)
                .map_err(|e| format!("pubblicazione non riuscita: {}", e.error))?;
            if sha(src).map_err(|e| e.to_string())? != sha(&dest).map_err(|e| e.to_string())? {
                let _ = std::fs::remove_file(&dest);
                return Err("verifica SHA-256 fallita".into());
            }
            Ok(dest)
        })();
        match result {
            Ok(p) => return Ok(p),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Consegna un verbale gia scritto in locale alla cartella condivisa configurata.
pub fn salva_su_condivisa(pdf: &Path, dir: Option<&Path>) -> Esito {
    salva_con_pausa(pdf, dir, Duration::from_secs(2))
}

fn salva_con_pausa(pdf: &Path, dir: Option<&Path>, pause: Duration) -> Esito {
    let Some(dir) = dir else {
        return Esito::NonConfigurata;
    };
    match copia_verificata(pdf, dir, pause) {
        Ok(p) => {
            let _ = std::fs::remove_file(pending_marker(pdf));
            Esito::Condivisa(p)
        }
        Err(e) => {
            let _ = std::fs::write(pending_marker(pdf), b"");
            Esito::SoloLocale(e)
        }
    }
}

/// Riprova i verbali rimasti in sospeso nella cartella locale. Ritorna (consegnati, ancora in sospeso).
pub fn riprova_sospesi(local: &Path, dir: Option<&Path>) -> (usize, usize) {
    let (mut ok, mut left) = (0, 0);
    let Ok(rd) = std::fs::read_dir(local) else {
        return (0, 0);
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "sospeso") {
            let pdf = p.with_extension("");
            if !pdf.exists() {
                let _ = std::fs::remove_file(&p);
            } else if matches!(salva_su_condivisa(&pdf, dir), Esito::Condivisa(_)) {
                ok += 1;
            } else {
                left += 1;
            }
        }
    }
    (ok, left)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_parsing() {
        assert_eq!(
            parse_config("report_dir = \"\\\\\\\\srv\\\\share\\\\r\"").unwrap(),
            PathBuf::from(r"\\srv\share\r")
        );
        assert_eq!(
            parse_config("# x\nreport_dir = 'J:\\Rep'").unwrap(),
            PathBuf::from(r"J:\Rep")
        );
        assert!(parse_config("altro = 1").is_none());
    }
    #[test]
    fn name_has_host_and_date() {
        let t = chrono::NaiveDate::from_ymd_opt(2026, 11, 5)
            .unwrap()
            .and_hms_opt(9, 30, 7)
            .unwrap();
        assert_eq!(
            file_name("WS ACC/1", t),
            "Verbale-TracciaDoc-WS-ACC-1-2026-11-05_093007.pdf"
        );
    }
    #[test]
    fn deliver_and_retry() {
        let tmp = tempfile::tempdir().unwrap();
        let pdf = tmp.path().join("v.pdf");
        std::fs::write(&pdf, b"%PDF-test").unwrap();
        assert_eq!(salva_su_condivisa(&pdf, None), Esito::NonConfigurata);
        // destinazione impossibile: un file al posto della cartella
        let blocker = tmp.path().join("blocco");
        std::fs::write(&blocker, b"x").unwrap();
        let r = salva_con_pausa(&pdf, Some(&blocker.join("sub")), Duration::ZERO);
        assert!(matches!(r, Esito::SoloLocale(_)));
        assert!(pending_marker(&pdf).exists());
        let share = tmp.path().join("share");
        let (ok, left) = riprova_sospesi(tmp.path(), Some(&share));
        assert_eq!((ok, left), (1, 0));
        assert_eq!(std::fs::read(share.join("v.pdf")).unwrap(), b"%PDF-test");
        assert!(!pending_marker(&pdf).exists());
        // stesso contenuto gia consegnato: idempotente
        assert!(matches!(
            salva_su_condivisa(&pdf, Some(&share)),
            Esito::Condivisa(_)
        ));
    }
}

/// Scrive il verbale nella cartella locale con nome automatico e lo consegna alla condivisa.
pub fn genera_e_salva(
    run: &crate::model::Run,
    paths: bool,
    demo: bool,
    cli_dir: Option<&str>,
) -> Result<(PathBuf, Esito), String> {
    let local = local_dir();
    std::fs::create_dir_all(&local)
        .map_err(|e| format!("Cartella locale dei verbali non creabile: {e}"))?;
    let pdf = local.join(file_name(&run.computer, chrono::Local::now().naive_local()));
    crate::report::write(run, &pdf, paths, demo)?;
    let esito = salva_su_condivisa(&pdf, report_dir(cli_dir).as_deref());
    Ok((pdf, esito))
}

/// Messaggio per l'operatore.
pub fn messaggio(pdf: &Path, esito: &Esito) -> String {
    let p = crate::diag::shown(pdf);
    match esito {
        Esito::Condivisa(_) => {
            format!("Verbale salvato e consegnato alla cartella condivisa (copia locale: {p})")
        }
        Esito::SoloLocale(e) => format!(
            "Cartella condivisa non raggiungibile ({e}): verbale salvato solo in locale in {p}. Verra riprovato all'apertura successiva."
        ),
        Esito::NonConfigurata => {
            format!("Verbale salvato in {p}. Nessuna cartella condivisa configurata.")
        }
    }
}
