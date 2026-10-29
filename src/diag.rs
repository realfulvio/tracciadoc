//! Registro eventi per la console della GUI e per "Copia diagnostica".
//! Mai contenuti dei documenti; il nome utente nei percorsi diventa `<utente>`.
use crate::model::Run;
use std::{
    collections::VecDeque,
    fs::OpenOptions,
    io::Write,
    path::Path,
    sync::{Mutex, OnceLock},
};

const MAX_EVENTS: usize = 500;
const COPIED_EVENTS: usize = 200;
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Info,
    Ok,
    Warn,
    Err,
}
impl Level {
    pub fn tag(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Ok => "OK  ",
            Level::Warn => "WARN",
            Level::Err => "ERR ",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Event {
    pub time: String,
    pub level: Level,
    pub text: String,
}
impl Event {
    pub fn line(&self) -> String {
        format!("{} {} {}", self.time, self.level.tag(), self.text)
    }
}
static EVENTS: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());

fn clock() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}
/// Sostituisce il nome utente in `C:\Users\<nome>\...` con `<utente>`.
pub fn mask(text: &str) -> String {
    const KEEP: [&str; 4] = ["public", "default", "all users", "default user"];
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(i) = lower[at..].find("\\users\\") {
        let name = at + i + 7;
        out.push_str(&text[at..name]);
        let end = text[name..].find(['\\', '/']).map_or(text.len(), |e| name + e);
        let segment = &text[name..end];
        if KEEP.contains(&segment.to_ascii_lowercase().as_str()) || segment.is_empty() {
            out.push_str(segment);
        } else {
            out.push_str("<utente>");
        }
        at = end;
    }
    out.push_str(&text[at..]);
    out
}
pub fn log(level: Level, text: impl AsRef<str>) {
    let event = Event {
        time: clock(),
        level,
        text: mask(text.as_ref()),
    };
    persist(&event);
    let mut events = EVENTS.lock().unwrap();
    if events.len() >= MAX_EVENTS {
        events.pop_front();
    }
    events.push_back(event);
}
pub fn info(text: impl AsRef<str>) {
    log(Level::Info, text);
}
pub fn ok(text: impl AsRef<str>) {
    log(Level::Ok, text);
}
pub fn warn(text: impl AsRef<str>) {
    log(Level::Warn, text);
}
/// Registra un avviso e lo restituisce, per usarlo direttamente come codice problema.
pub fn noted(text: &str) -> &str {
    warn(text);
    text
}
pub fn err(text: impl AsRef<str>) {
    log(Level::Err, text);
}
/// Ultimi `n` eventi, dal piu vecchio.
pub fn recent(n: usize) -> Vec<Event> {
    let events = EVENTS.lock().unwrap();
    events.iter().skip(events.len().saturating_sub(n)).cloned().collect()
}
/// Mette il testo negli appunti di Windows (UTF-16). false se gli appunti sono occupati o non disponibili.
pub fn copy_to_clipboard(text: &str) -> bool {
    use windows_sys::Win32::System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock},
    };
    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.replace("\r\n", "\n").replace('\n', "\r\n").encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        if !opened {
            return false;
        }
        let bytes = wide.len() * 2;
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
        let mut done = false;
        if !handle.is_null() {
            let target = GlobalLock(handle) as *mut u16;
            if !target.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
                GlobalUnlock(handle);
                EmptyClipboard();
                // In caso di successo la memoria passa agli appunti.
                done = !SetClipboardData(CF_UNICODETEXT, handle).is_null();
            }
            if !done {
                GlobalFree(handle);
            }
        }
        CloseClipboard();
        done
    }
}
/// Aggiunge la riga al file di log; se supera il limite riparte da zero. Errori ignorati: il log non deve fermare la verifica.
fn persist(event: &Event) {
    if cfg!(test) {
        return;
    }
    let dir = crate::engine::state_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("diagnostica.log");
    let mut options = OpenOptions::new();
    options.create(true);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        options.write(true).truncate(true);
    } else {
        options.append(true);
    }
    if let Ok(mut file) = options.open(&path) {
        let _ = writeln!(file, "{} {}", chrono::Local::now().format("%Y-%m-%d"), event.line());
    }
}
pub fn log_path() -> std::path::PathBuf {
    crate::engine::state_dir().join("diagnostica.log")
}

fn os_version() -> &'static str {
    static OS: OnceLock<String> = OnceLock::new();
    OS.get_or_init(|| {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .creation_flags(0x0800_0000)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Windows (versione non letta)".into())
    })
}
fn ram_gb() -> f64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return 0.;
    }
    status.ullTotalPhys as f64 / (1u64 << 30) as f64
}
fn cpu_features() -> String {
    let mut features = vec![];
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            features.push("AVX2");
        }
        if is_x86_feature_detected!("avx512f") {
            features.push("AVX-512");
        }
        if is_x86_feature_detected!("sse4.2") {
            features.push("SSE4.2");
        }
    }
    if features.is_empty() {
        "nessuna estensione rilevata".into()
    } else {
        features.join(" ")
    }
}
/// Testo pronto da incollare agli sviluppatori.
pub fn report(gpu: &str, phase: &str, current: &str, run: Option<&Run>) -> String {
    let mut t = String::new();
    let mut line = |s: String| {
        t.push_str(&s);
        t.push('\n');
    };
    line("=== TracciaDoc diagnostica ===".into());
    line(format!(
        "Versione {} · generata {}",
        env!("CARGO_PKG_VERSION"),
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    line(format!(
        "Sistema: {} · RAM {:.1} GB · CPU {} · {} thread ({})",
        os_version(),
        ram_gb(),
        std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "?".into()),
        std::thread::available_parallelism().map_or(1, |n| n.get()),
        cpu_features()
    ));
    line(format!("Grafica GUI: {gpu}"));
    match run {
        None => line("Fase: nessuna verifica avviata".into()),
        Some(r) => {
            line(format!(
                "OCR: ita+eng · worker {} · solo CPU {} · prefiltro {} · backend {}",
                r.settings.workers,
                r.settings.cpu_only,
                r.settings.prefilter,
                if r.backend.is_empty() { "-" } else { &r.backend }
            ));
            line(format!(
                "Fase: {} · {} · {}",
                if phase.is_empty() { "-" } else { phase },
                if r.finished { "terminata" } else { "in corso" },
                if r.interrupted { "INTERROTTA" } else { "non interrotta" }
            ));
            if !current.is_empty() {
                line(format!("File corrente: {}", mask(current)));
            }
            line(format!(
                "Aree: {}",
                r.roots.iter().map(|p| mask(&p.display().to_string())).collect::<Vec<_>>().join(" · ")
            ));
            line(format!(
                "Contatori: visti {} · analizzati {} · parziali {} · saltati {} · segnalati {} · eliminati {} · pagine OCR {} · {:.1} s",
                r.encountered,
                r.analyzed,
                r.partial_files,
                r.skipped,
                r.findings.len(),
                r.deleted_count(),
                r.pages_ocr,
                r.elapsed_secs
            ));
            if r.previous.is_some() {
                line("Tipo: ri-scansione di verifica finale".into());
            }
            if r.issues.is_empty() {
                line("Problemi: nessuno".into());
            } else {
                line(format!(
                    "Problemi: {}",
                    r.issues.iter().map(|(k, n)| format!("{k} x{n}")).collect::<Vec<_>>().join(" · ")
                ));
            }
            let failed: Vec<_> = r.findings.iter().filter(|f| !f.delete_error.is_empty()).collect();
            if !failed.is_empty() {
                line("--- Eliminazioni non riuscite ---".into());
                for f in failed {
                    line(format!("{} · {}", mask(&f.path.display().to_string()), f.delete_error));
                }
            }
        }
    }
    line(format!("--- Ultimi eventi (max {COPIED_EVENTS}) ---"));
    for e in recent(COPIED_EVENTS) {
        line(e.line());
    }
    line(format!("Log completo: {}", mask(&log_path().display().to_string())));
    line("Contenuto dei documenti: mai incluso · nome utente nei percorsi sostituito con <utente>".into());
    t
}
/// Percorso di un file con nome leggibile e nome utente mascherato, per i messaggi di errore.
pub fn shown(path: &Path) -> String {
    mask(&path.display().to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn masks_user_name() {
        assert_eq!(mask(r"C:\Users\mario.rossi\Desktop\a.pdf"), r"C:\Users\<utente>\Desktop\a.pdf");
        assert_eq!(mask(r"c:\USERS\Mario"), r"c:\USERS\<utente>");
        assert_eq!(mask(r"C:\Users\Public\x"), r"C:\Users\Public\x");
        assert_eq!(mask(r"D:\Scansioni\a.pdf"), r"D:\Scansioni\a.pdf");
        assert_eq!(mask(r"C:\Users\a\x e C:\Users\b\y"), r"C:\Users\<utente>\x e C:\Users\<utente>\y");
    }
    #[test]
    fn keeps_last_events_only() {
        for i in 0..MAX_EVENTS + 20 {
            log(Level::Info, format!("evento {i}"));
        }
        assert!(recent(usize::MAX).len() <= MAX_EVENTS);
        assert!(recent(1)[0].text.ends_with(&format!("{}", MAX_EVENTS + 19)));
    }
}
