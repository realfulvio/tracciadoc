//! Aggiornamento dalle release pubbliche su GitHub. Scarica con curl.exe di Windows (HTTPS,
//! nessuna dipendenza in più), verifica lo SHA-256 di SHA256SUMS.txt e sostituisce i file
//! solo quando nessuna scansione è in corso. Dal PC esce solo la richiesta dell'elenco release.
use crate::{diag, install, sched};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const API: &str = "https://api.github.com/repos/realfulvio/tracciadoc/releases/latest";

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub zip_name: String,
    pub zip_url: String,
    pub sums_url: String,
}
/// "0.4.1" -> (0, 4, 1); confronto numerico, non alfabetico.
pub fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let mut it = v.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u32>().ok());
    Some((it.next()??, it.next()??, it.next().flatten().unwrap_or(0)))
}
pub fn newer(remote: &str, local: &str) -> bool {
    matches!((parse_version(remote), parse_version(local)), (Some(r), Some(l)) if r > l)
}
fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    let mut cmd = std::process::Command::new(PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("System32\\curl.exe"));
    cmd.args(["-fsSL", "--proto", "=https", "--max-time", "300", "-A", concat!("TracciaDoc/", env!("CARGO_PKG_VERSION"))]).args(args);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let out = cmd.output().map_err(|e| format!("curl non avviato: {e}"))?;
    if out.status.success() { Ok(out.stdout) } else { Err(format!("download non riuscito: {}", String::from_utf8_lossy(&out.stderr).trim())) }
}
/// Ultima release pubblicata, se più recente di questa.
pub fn check() -> Result<Option<Release>, String> {
    let body = curl(&["-H", "Accept: application/vnd.github+json", API])?;
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|_| "Risposta delle release non valida")?;
    let tag = v["tag_name"].as_str().ok_or("Release senza versione")?;
    if !newer(tag, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let assets = v["assets"].as_array().ok_or("Release senza file")?;
    let find = |pred: &dyn Fn(&str) -> bool| assets.iter().find(|a| a["name"].as_str().is_some_and(pred)).map(|a| (a["name"].as_str().unwrap_or("").to_string(), a["browser_download_url"].as_str().unwrap_or("").to_string()));
    let (zip_name, zip_url) = find(&|n: &str| n.ends_with("-windows-x64.zip")).ok_or("Zip della release mancante")?;
    let (_, sums_url) = find(&|n: &str| n == "SHA256SUMS.txt").ok_or("SHA256SUMS.txt mancante: aggiornamento non verificabile")?;
    let https = |u: &str| u.starts_with("https://github.com/realfulvio/tracciadoc/");
    if !https(&zip_url) || !https(&sums_url) {
        return Err("Indirizzo di download inatteso".into());
    }
    Ok(Some(Release { version: tag.trim_start_matches('v').to_string(), zip_name, zip_url, sums_url }))
}
/// Hash atteso per `name` da un file SHA256SUMS ("<hash>  <nome>").
pub fn expected_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (h, n) = l.trim().split_once(char::is_whitespace)?;
        (n.trim().trim_start_matches('*') == name && h.len() == 64).then(|| h.to_ascii_lowercase())
    })
}
fn sha256(path: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", h.finalize()))
}
/// Estrae lo zip in `dest` rifiutando percorsi che escono dalla cartella.
fn extract(zip: &Path, dest: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip).map_err(|e| e.to_string())?).map_err(|e| format!("Zip non valido: {e}"))?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
        let rel = f.enclosed_name().ok_or("Percorso non sicuro nello zip")?;
        let out = dest.join(rel);
        if f.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(d) = out.parent() {
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
            }
            let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut w).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
/// Cartella dello zip che contiene TracciaDoc.exe (alla radice o in una sottocartella).
fn package_root(dir: &Path) -> Option<PathBuf> {
    walkdir::WalkDir::new(dir).max_depth(2).into_iter().filter_map(Result::ok).find(|e| e.file_name().eq_ignore_ascii_case("TracciaDoc.exe")).and_then(|e| e.path().parent().map(Path::to_path_buf))
}
/// Versione dichiarata dall'EXE scaricato (`--version`): controllo che sia davvero eseguibile.
/// Le versioni precedenti alla 0.5.0 non conoscono `--version`: dopo 20 s il processo viene chiuso.
fn exe_version(exe: &Path) -> Option<String> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    use std::io::Read;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--version").stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let mut child = cmd.spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        if child.try_wait().ok()?.is_some() {
            break;
        }
        if start.elapsed() > std::time::Duration::from_secs(20) {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out.trim().to_string())
}
fn preflight() -> Result<(), String> {
    if !install::is_admin() {
        return Err("Serve l'amministratore".into());
    }
    if !install::is_installed() {
        return Err("TracciaDoc non è installato su questo PC".into());
    }
    install::cleanup_old();
    Ok(())
}
/// `--update`: scarica l'ultima release, verifica e installa. Solo sulla copia installata e da amministratore.
pub fn apply(auto: bool) -> Result<String, String> {
    preflight()?;
    if auto && sched::load_config().update != sched::UpdateMode::Auto {
        return Ok("Aggiornamento automatico disattivato".into());
    }
    let Some(rel) = check()? else {
        return Ok("Nessun aggiornamento: versione già aggiornata".into());
    };
    if sched::scan_running() {
        return Ok(format!("{} rimandato: scansione in corso", rel.version));
    }
    let work = sched::data_dir().join("aggiornamenti");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let zip = work.join(&rel.zip_name);
    let sums = String::from_utf8(curl(&[&rel.sums_url])?).map_err(|_| "SHA256SUMS.txt non leggibile")?;
    let expected = expected_hash(&sums, &rel.zip_name).ok_or("Hash dello zip assente da SHA256SUMS.txt")?;
    curl(&["-o", &zip.to_string_lossy(), &rel.zip_url])?;
    let result = install_package(&zip, &expected, Some(&rel.version), &work);
    let _ = std::fs::remove_dir_all(&work);
    result
}
/// `--update-from <zip>`: aggiorna da uno zip già scaricato (PC senza Internet o collaudo).
/// Serve SHA256SUMS.txt accanto allo zip, come nelle release.
pub fn apply_from(zip: &Path) -> Result<String, String> {
    preflight()?;
    let name = zip.file_name().ok_or("Zip non valido")?.to_string_lossy().to_string();
    let sums = std::fs::read_to_string(zip.with_file_name("SHA256SUMS.txt")).map_err(|_| "Manca SHA256SUMS.txt accanto allo zip")?;
    let expected = expected_hash(&sums, &name).ok_or("Hash dello zip assente da SHA256SUMS.txt")?;
    if sched::scan_running() {
        return Err("Scansione in corso: riprova al termine".into());
    }
    let work = sched::data_dir().join("aggiornamenti");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let result = install_package(zip, &expected, None, &work);
    let _ = std::fs::remove_dir_all(&work);
    result
}
/// Verifica hash ed EXE, poi sostituisce i file installati. `version` None = qualsiasi versione più recente.
fn install_package(zip: &Path, expected: &str, version: Option<&str>, work: &Path) -> Result<String, String> {
    let got = sha256(zip)?;
    if got != expected {
        return Err("SHA-256 non corrispondente: aggiornamento scartato".into());
    }
    let staging = work.join("estratto");
    extract(zip, &staging)?;
    let root = package_root(&staging).ok_or("TracciaDoc.exe assente dallo zip")?;
    let declared = exe_version(&root.join("TracciaDoc.exe")).unwrap_or_default();
    let ok = match version {
        Some(v) => declared == v,
        None => newer(&declared, env!("CARGO_PKG_VERSION")),
    };
    if !ok {
        return Err(format!("L'EXE dello zip dichiara la versione '{declared}': aggiornamento scartato"));
    }
    // Ricontrolla subito prima di sostituire: una scansione potrebbe essere partita nel frattempo.
    if sched::scan_running() {
        return Ok(format!("{declared} rimandato: scansione in corso"));
    }
    install::replace_tree(&root, &install::install_dir())?;
    // Via subito le copie .old; quelle ancora in uso (icona aperta) restano fino alla prossima pulizia.
    install::cleanup_old();
    install::register(&install::installed_exe())?;
    // Ricrea le attività con l'EXE nuovo (stesso percorso, eventuali nuove opzioni).
    let _ = sched::apply_config(&sched::load_config(), &install::installed_exe());
    let msg = format!("Aggiornato da {} a {declared} (SHA-256 verificato)", env!("CARGO_PKG_VERSION"));
    diag::ok(&msg);
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_compare_numerically() {
        assert!(newer("v0.10.0", "0.9.9"));
        assert!(newer("0.5.0", "0.4.1"));
        assert!(!newer("v0.4.1", "0.4.1"));
        assert!(!newer("garbage", "0.4.1"));
    }
    #[test]
    fn sums_lookup() {
        let sums = format!("{}  TracciaDoc-0.5.0-windows-x64.zip\n{}  altro.txt", "A".repeat(64), "b".repeat(64));
        assert_eq!(expected_hash(&sums, "TracciaDoc-0.5.0-windows-x64.zip"), Some("a".repeat(64)));
        assert_eq!(expected_hash(&sums, "manca.zip"), None);
    }
}
