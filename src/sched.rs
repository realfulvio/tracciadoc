//! Scansioni pianificate: piano scelto nella GUI, attività dell'Utilità di pianificazione
//! di Windows (account di sistema), stato condiviso con l'icona nell'area di notifica.
use crate::{diag, engine, model::Settings};
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub const TASK_SCAN: &str = "TracciaDoc\\Scansione pianificata";
pub const TASK_UPDATE: &str = "TracciaDoc\\Aggiornamento";
/// Mutex globale tenuto da ogni scansione (GUI o pianificata): l'aggiornamento non sostituisce l'EXE se esiste.
pub const SCAN_MUTEX: &str = "Global\\TracciaDocScansione";
/// Secondi di avviso prima dello spegnimento a fine scansione.
pub const SHUTDOWN_DELAY: u64 = 60;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Repeat {
    Once,
    Daily,
    #[default]
    Weekly,
    Monthly,
}
impl Repeat {
    pub fn label(self) -> &'static str {
        match self {
            Repeat::Once => "Una volta",
            Repeat::Daily => "Ogni giorno",
            Repeat::Weekly => "Ogni settimana",
            Repeat::Monthly => "Ogni mese",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateMode {
    /// Scarica e installa da solo (attività di sistema), mai durante una scansione.
    #[default]
    Auto,
    /// Mostra soltanto il pallino blu e la notifica.
    Notify,
    Off,
}
impl UpdateMode {
    pub fn label(self) -> &'static str {
        match self {
            UpdateMode::Auto => "Automatico",
            UpdateMode::Notify => "Solo avviso",
            UpdateMode::Off => "Disattivato",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub repeat: Repeat,
    /// Lunedì..domenica, usato con Repeat::Weekly.
    pub days: [bool; 7],
    /// Giorno del mese (1..=31) per Repeat::Monthly; i mesi più corti vengono saltati.
    pub day_of_month: u32,
    pub hour: u32,
    pub minute: u32,
    /// Data di inizio AAAA-MM-GG.
    pub start: String,
    /// Data di fine AAAA-MM-GG, vuota = senza scadenza.
    #[serde(default)]
    pub end: String,
    /// Arresta il PC a fine scansione, solo se completa e con il verbale salvato.
    pub shutdown: bool,
}
impl Default for Plan {
    fn default() -> Self {
        Self {
            repeat: Repeat::Weekly,
            days: [true, false, false, false, false, false, false],
            day_of_month: 1,
            hour: 13,
            minute: 30,
            start: Local::now().date_naive().format("%Y-%m-%d").to_string(),
            end: String::new(),
            shutdown: true,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub plan: Option<Plan>,
    #[serde(default)]
    pub update: UpdateMode,
}

/// Date in formato AAAA-MM-GG o GG/MM/AAAA.
pub fn parse_date(text: &str) -> Option<NaiveDate> {
    let t = text.trim();
    NaiveDate::parse_from_str(t, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(t, "%d/%m/%Y"))
        .ok()
}
impl Plan {
    pub fn validate(&self) -> Result<(NaiveDate, Option<NaiveDate>, NaiveTime), String> {
        let start = parse_date(&self.start).ok_or("Data di inizio non valida (GG/MM/AAAA)")?;
        let end = if self.end.trim().is_empty() {
            None
        } else {
            Some(parse_date(&self.end).ok_or("Data di fine non valida (GG/MM/AAAA)")?)
        };
        if end.is_some_and(|e| e < start) {
            return Err("La data di fine è precedente a quella di inizio".into());
        }
        let time = NaiveTime::from_hms_opt(self.hour, self.minute, 0).ok_or("Orario non valido")?;
        if self.repeat == Repeat::Weekly && !self.days.iter().any(|d| *d) {
            return Err("Scegli almeno un giorno della settimana".into());
        }
        if self.repeat == Repeat::Monthly && !(1..=31).contains(&self.day_of_month) {
            return Err("Giorno del mese non valido".into());
        }
        if self.repeat == Repeat::Once && NaiveDateTime::new(start, time) <= Local::now().naive_local() {
            return Err("La data e l'ora scelte sono già passate".into());
        }
        Ok((start, end, time))
    }
    /// Prossime `n` esecuzioni dopo `from` (ora locale).
    pub fn next_runs(&self, from: NaiveDateTime, n: usize) -> Vec<NaiveDateTime> {
        let Ok((start, end, time)) = self.validate_dates() else { return vec![] };
        let mut out = vec![];
        let mut day = start.max(from.date());
        for _ in 0..800 {
            if out.len() >= n || end.is_some_and(|e| day > e) {
                break;
            }
            let at = NaiveDateTime::new(day, time);
            let hit = match self.repeat {
                Repeat::Once => day == start,
                Repeat::Daily => true,
                Repeat::Weekly => self.days[day.weekday().num_days_from_monday() as usize],
                Repeat::Monthly => day.day() == self.day_of_month,
            };
            if hit && at > from {
                out.push(at);
            }
            if self.repeat == Repeat::Once && day >= start {
                break;
            }
            day = day.succ_opt().unwrap_or(day);
        }
        out
    }
    /// Come validate() ma senza il controllo "già passata" (serve per l'elenco).
    fn validate_dates(&self) -> Result<(NaiveDate, Option<NaiveDate>, NaiveTime), ()> {
        let start = parse_date(&self.start).ok_or(())?;
        let end = if self.end.trim().is_empty() { None } else { Some(parse_date(&self.end).ok_or(())?) };
        let time = NaiveTime::from_hms_opt(self.hour, self.minute, 0).ok_or(())?;
        Ok((start, end, time))
    }
    pub fn summary(&self) -> String {
        let at = format!("{:02}:{:02}", self.hour, self.minute);
        match self.repeat {
            Repeat::Once => format!("una volta, {} alle {at}", parse_date(&self.start).map_or(self.start.clone(), |d| d.format("%d/%m/%Y").to_string())),
            Repeat::Daily => format!("ogni giorno alle {at}"),
            Repeat::Weekly => {
                let names: Vec<&str> = DAYS.iter().zip(self.days).filter(|(_, on)| *on).map(|(d, _)| *d).collect();
                format!("ogni settimana ({}) alle {at}", names.join(", "))
            }
            Repeat::Monthly => format!("ogni mese il giorno {} alle {at}", self.day_of_month),
        }
    }
}
pub const DAYS: [&str; 7] = ["Lun", "Mar", "Mer", "Gio", "Ven", "Sab", "Dom"];
const DAYS_XML: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
pub fn it_weekday(w: Weekday) -> &'static str {
    ["lun", "mar", "mer", "gio", "ven", "sab", "dom"][w.num_days_from_monday() as usize]
}
pub fn when_text(t: NaiveDateTime) -> String {
    format!("{} {}", it_weekday(t.weekday()), t.format("%d/%m · %H:%M"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
/// Definizione XML dell'attività: account di sistema, recupero se il PC era spento, priorità bassa.
pub fn scan_task_xml(plan: &Plan, exe: &Path) -> Result<String, String> {
    let (start, end, time) = plan.validate_dates().map_err(|_| "Piano non valido".to_string())?;
    let boundary = format!("<StartBoundary>{}T{}</StartBoundary>", start.format("%Y-%m-%d"), time.format("%H:%M:%S"));
    let end = end.map_or(String::new(), |e| format!("<EndBoundary>{}T23:59:59</EndBoundary>", e.format("%Y-%m-%d")));
    let trigger = match plan.repeat {
        Repeat::Once => format!("<TimeTrigger>{boundary}{end}<Enabled>true</Enabled></TimeTrigger>"),
        Repeat::Daily => format!("<CalendarTrigger>{boundary}{end}<Enabled>true</Enabled><ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay></CalendarTrigger>"),
        Repeat::Weekly => {
            let days: String = DAYS_XML.iter().zip(plan.days).filter(|(_, on)| *on).map(|(d, _)| format!("<{d} />")).collect();
            format!("<CalendarTrigger>{boundary}{end}<Enabled>true</Enabled><ScheduleByWeek><WeeksInterval>1</WeeksInterval><DaysOfWeek>{days}</DaysOfWeek></ScheduleByWeek></CalendarTrigger>")
        }
        Repeat::Monthly => {
            let months: String = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"].iter().map(|m| format!("<{m} />")).collect();
            format!("<CalendarTrigger>{boundary}{end}<Enabled>true</Enabled><ScheduleByMonth><DaysOfMonth><Day>{}</Day></DaysOfMonth><Months>{months}</Months></ScheduleByMonth></CalendarTrigger>", plan.day_of_month)
        }
    };
    Ok(task_xml("Scansione pianificata dei dischi fissi: salva il verbale, non cancella nulla.", &trigger, exe, "--scheduled", "PT12H"))
}
/// Aggiornamento automatico: 10 minuti dopo l'avvio e ogni giorno alle 12:00 (recuperato se il PC era spento).
pub fn update_task_xml(exe: &Path) -> String {
    let today = Local::now().date_naive().format("%Y-%m-%d");
    let trigger = format!("<BootTrigger><Enabled>true</Enabled><Delay>PT10M</Delay></BootTrigger><CalendarTrigger><StartBoundary>{today}T12:00:00</StartBoundary><Enabled>true</Enabled><ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay></CalendarTrigger>");
    task_xml("Controlla e installa gli aggiornamenti di TracciaDoc (mai durante una scansione).", &trigger, exe, "--update --auto", "PT1H")
}
fn task_xml(description: &str, triggers: &str, exe: &Path, args: &str, limit: &str) -> String {
    let exe = xml_escape(&exe.to_string_lossy());
    let dir = xml_escape(&exe_dir_of(&exe));
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Author>TracciaDoc</Author><Description>{description}</Description></RegistrationInfo>
  <Triggers>{triggers}</Triggers>
  <Principals><Principal id="Author"><UserId>S-1-5-18</UserId><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>{limit}</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>"{exe}"</Command><Arguments>{args}</Arguments><WorkingDirectory>{dir}</WorkingDirectory></Exec></Actions>
</Task>
"#
    )
}
fn exe_dir_of(exe: &str) -> String {
    exe.rsplit_once('\\').map_or(String::new(), |(d, _)| d.to_string())
}

/// %ProgramData%\TracciaDoc: configurazione, stato e verbali delle scansioni pianificate.
pub fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("ProgramData").unwrap_or_else(|| "C:\\ProgramData".into())).join("TracciaDoc")
}
pub fn verbali_dir() -> PathBuf {
    data_dir().join("Verbali")
}
fn config_path() -> PathBuf {
    data_dir().join("config.json")
}
fn status_path() -> PathBuf {
    data_dir().join("stato.json")
}
pub fn last_run_path() -> PathBuf {
    data_dir().join("ultima-pianificata.json")
}
/// File creato dall'utente (tray o GUI) per annullare lo spegnimento in attesa.
pub fn cancel_flag() -> PathBuf {
    data_dir().join("annulla-spegnimento")
}
/// Marcatore per utente "esito già rivisto" (scritto dalla GUI quando apre la revisione).
fn seen_marker(id: &str) -> PathBuf {
    let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    engine::state_dir().join(format!("visto-{safe}"))
}
pub fn seen(id: &str) -> bool {
    seen_marker(id).exists()
}
pub fn mark_seen(id: &str) {
    let _ = std::fs::create_dir_all(engine::state_dir());
    let _ = std::fs::write(seen_marker(id), b"");
}
pub fn load_config() -> Config {
    std::fs::read(config_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Entry {
    /// Ora locale di inizio (AAAA-MM-GG HH:MM).
    pub when: String,
    /// clean | found | incomplete | error
    pub outcome: String,
    pub findings: usize,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub note: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Status {
    /// idle | scanning
    pub state: String,
    pub percent: u8,
    /// Spegnimento previsto (secondi Unix), 0 se nessuno.
    #[serde(default)]
    pub shutdown_at: i64,
    #[serde(default)]
    pub history: Vec<Entry>,
    /// Ultimo esito dell'aggiornamento automatico.
    #[serde(default)]
    pub update_note: String,
}
pub fn load_status() -> Status {
    std::fs::read(status_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn save_status(s: &Status) {
    if let Err(e) = write_atomic(&status_path(), &serde_json::to_vec_pretty(s).unwrap_or_default()) {
        diag::warn(format!("Stato della pianificazione non scritto: {e}"));
    }
}
pub fn set_update_note(note: &str) {
    let mut s = load_status();
    s.update_note = format!("{} · {note}", Local::now().format("%d/%m %H:%M"));
    save_status(&s);
}

fn system32(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("System32").join(name)
}
/// Esegue un programma di sistema senza finestra; errore con l'output se il codice non è 0.
pub fn run_tool(name: &str, args: &[&str]) -> Result<String, String> {
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new(system32(name));
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output().map_err(|e| format!("{name} non avviato: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    if out.status.success() { Ok(text) } else { Err(format!("{name}: {}", text.trim())) }
}
/// schtasks /XML vuole un file UTF-16 LE con BOM.
fn register_task(name: &str, xml: &str) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("tracciadoc-task-{}.xml", std::process::id()));
    let mut bytes = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(&path, bytes).map_err(|e| format!("Definizione attività non scritta: {e}"))?;
    let result = run_tool("schtasks.exe", &["/Create", "/TN", name, "/XML", &path.to_string_lossy(), "/F"]);
    let _ = std::fs::remove_file(&path);
    result.map(|_| ())
}
pub fn delete_task(name: &str) {
    let _ = run_tool("schtasks.exe", &["/Delete", "/TN", name, "/F"]);
}
/// Applica la configurazione (serve l'amministratore): salva config.json e crea/rimuove le attività.
pub fn apply_config(cfg: &Config, exe: &Path) -> Result<(), String> {
    if let Some(plan) = &cfg.plan {
        plan.validate()?;
    }
    std::fs::create_dir_all(verbali_dir()).map_err(|e| format!("Cartella dati non creata: {e}"))?;
    match &cfg.plan {
        Some(plan) => register_task(TASK_SCAN, &scan_task_xml(plan, exe)?)?,
        None => delete_task(TASK_SCAN),
    }
    match cfg.update {
        UpdateMode::Auto => register_task(TASK_UPDATE, &update_task_xml(exe))?,
        _ => delete_task(TASK_UPDATE),
    }
    write_atomic(&config_path(), &serde_json::to_vec_pretty(cfg).unwrap_or_default()).map_err(|e| format!("Configurazione non scritta: {e}"))?;
    diag::ok(format!("Pianificazione applicata: {}", cfg.plan.as_ref().map_or("nessuna".into(), |p| p.summary())));
    Ok(())
}

/// Mutex di sistema: Some finché la scansione è in corso. None se un'altra scansione lo tiene già.
pub struct ScanLock(#[allow(dead_code)] isize);
impl Drop for ScanLock {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0 as _);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
#[cfg(windows)]
pub fn scan_lock() -> Option<ScanLock> {
    use windows_sys::Win32::{Foundation::{ERROR_ALREADY_EXISTS, GetLastError}, System::Threading::CreateMutexW};
    let name = wide(SCAN_MUTEX);
    let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if h.is_null() {
        return None;
    }
    let lock = ScanLock(h as isize);
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS { None } else { Some(lock) }
}
#[cfg(not(windows))]
pub fn scan_lock() -> Option<ScanLock> {
    Some(ScanLock(0))
}
#[cfg(windows)]
pub fn scan_running() -> bool {
    use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE}};
    let name = wide(SCAN_MUTEX);
    let h = unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
    if h.is_null() {
        return false;
    }
    unsafe { CloseHandle(h) };
    true
}
#[cfg(not(windows))]
pub fn scan_running() -> bool {
    false
}
#[cfg(windows)]
fn low_priority() {
    use windows_sys::Win32::System::Threading::{BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, SetPriorityClass};
    // I worker OCR sono processi figli: ereditano la priorità "inferiore al normale".
    unsafe { SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) };
}
#[cfg(not(windows))]
fn low_priority() {}

/// Verbale nella cartella locale (per SYSTEM: ProgramData\TracciaDoc\Verbali) e consegna
/// alla cartella condivisa configurata, con nuovo tentativo dei verbali rimasti in sospeso.
pub fn save_report(run: &crate::model::Run) -> Result<PathBuf, String> {
    let dir = crate::shared::report_dir(None);
    let (pdf, esito) = crate::shared::genera_e_salva(run, true, false, None)?;
    let (ok, _) = crate::shared::riprova_sospesi(&crate::shared::local_dir(), dir.as_deref());
    if ok > 0 {
        diag::ok(format!("Consegnati {ok} verbali rimasti in sospeso"));
    }
    match &esito {
        crate::shared::Esito::SoloLocale(e) => diag::warn(format!("Verbale solo in locale: {e}")),
        _ => diag::info(crate::shared::messaggio(&pdf, &esito)),
    }
    Ok(pdf)
}
/// Dischi fissi con lettera, come la selezione predefinita della GUI.
pub fn fixed_roots() -> Vec<PathBuf> {
    engine::volumes()
        .into_iter()
        .filter(|(p, l)| l.ends_with("locale fisso") && !p.to_string_lossy().starts_with("\\\\?\\Volume"))
        .map(|(p, _)| p)
        .collect()
}

/// `--scheduled`: scansione senza finestra lanciata dall'Utilità di pianificazione. Non cancella nulla.
pub fn run_scheduled() -> Result<(), String> {
    let Some(_lock) = scan_lock() else {
        diag::warn("Scansione pianificata saltata: un'altra scansione è già in corso");
        return Ok(());
    };
    low_priority();
    let cfg = load_config();
    let shutdown = cfg.plan.as_ref().is_some_and(|p| p.shutdown);
    let when = Local::now().format("%Y-%m-%d %H:%M").to_string();
    let roots = fixed_roots();
    if roots.is_empty() {
        record(Entry { when, outcome: "error".into(), note: "nessun disco fisso".into(), ..Default::default() }, 0);
        return Err("Nessun disco fisso trovato".into());
    }
    diag::info(format!("Scansione pianificata avviata su {} dischi", roots.len()));
    let run = engine::new_run(roots, Settings::default(), "Scansione pianificata".into(), "Pianificazione automatica".into());
    let shared = Arc::new(Mutex::new(engine::Progress { run: Some(run), ..Default::default() }));
    let stop = Arc::new(AtomicBool::new(false));
    let mut status = load_status();
    status.state = "scanning".into();
    status.percent = 0;
    status.shutdown_at = 0;
    save_status(&status);
    let done = Arc::new(AtomicBool::new(false));
    let progress = {
        let (shared, done) = (shared.clone(), done.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(5));
                let pct = {
                    let p = shared.lock().unwrap();
                    let (d, t) = (p.counts.done.load(Ordering::Relaxed), p.counts.total.load(Ordering::Relaxed));
                    (d * 100).checked_div(t).map_or(0, |p| p.min(99) as u8)
                };
                let mut s = load_status();
                s.state = "scanning".into();
                s.percent = pct;
                save_status(&s);
            }
        })
    };
    engine::run(shared.clone(), stop);
    done.store(true, Ordering::Relaxed);
    let _ = progress.join();
    let run = shared.lock().unwrap().run.clone().ok_or("Esito della scansione mancante")?;
    let _ = engine::audit(&run, "scheduled");
    // Esito completo per la revisione dalla GUI (percorsi solo su questo PC).
    if let Err(e) = write_atomic(&last_run_path(), &serde_json::to_vec(&run).unwrap_or_default()) {
        diag::warn(format!("Esito pianificato non salvato: {e}"));
    }
    let report = save_report(&run);
    let complete = !run.incomplete() && !run.interrupted;
    let outcome = match (&report, complete, run.findings.len()) {
        (Err(_), _, _) => "error",
        (_, false, _) => "incomplete",
        (_, true, 0) => "clean",
        _ => "found",
    };
    let note = match &report {
        Ok(p) => {
            diag::ok(format!("Verbale pianificato salvato: {}", diag::shown(p)));
            String::new()
        }
        Err(e) => {
            diag::err(format!("Verbale pianificato non scritto: {e}"));
            e.clone()
        }
    };
    let entry = Entry { when, outcome: outcome.into(), findings: run.findings.len(), id: run.id.clone(), note };
    let off = shutdown && report.is_ok() && complete;
    record(entry, if off { chrono::Utc::now().timestamp() + SHUTDOWN_DELAY as i64 } else { 0 });
    if shutdown && !off {
        diag::warn("Spegnimento non eseguito: scansione incompleta o verbale non salvato");
    }
    if off {
        shutdown_after_countdown();
    }
    report.map(|_| ())
}
fn record(entry: Entry, shutdown_at: i64) {
    let mut s = load_status();
    s.state = "idle".into();
    s.percent = 0;
    s.shutdown_at = shutdown_at;
    s.history.insert(0, entry);
    s.history.truncate(20);
    save_status(&s);
}
/// Attende l'avviso (annullabile dall'icona o dalla GUI), poi chiede l'arresto senza forzare
/// la chiusura dei programmi: con /t 0 e senza /f le app con dati non salvati possono bloccarlo.
pub fn shutdown_after_countdown() {
    let _ = std::fs::remove_file(cancel_flag());
    for _ in 0..SHUTDOWN_DELAY {
        std::thread::sleep(Duration::from_secs(1));
        if cancel_flag().exists() {
            let _ = std::fs::remove_file(cancel_flag());
            diag::info("Spegnimento annullato dall'utente");
            clear_shutdown();
            return;
        }
    }
    clear_shutdown();
    match run_tool("shutdown.exe", &["/s", "/t", "0", "/d", "p:0:0"]) {
        Ok(_) => diag::info("Spegnimento richiesto a fine scansione"),
        Err(e) => diag::err(format!("Spegnimento non riuscito: {e}")),
    }
}
fn clear_shutdown() {
    let mut s = load_status();
    s.shutdown_at = 0;
    save_status(&s);
}
pub fn cancel_shutdown() {
    let _ = std::fs::write(cancel_flag(), b"1");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }
    #[test]
    fn weekly_next_runs() {
        let plan = Plan { start: "2026-11-05".into(), ..Default::default() };
        // 5/11/2026 è giovedì: il primo lunedì utile è il 9.
        let runs = plan.next_runs(at("2026-11-05 08:00"), 3);
        assert_eq!(runs, vec![at("2026-11-09 13:30"), at("2026-11-16 13:30"), at("2026-11-23 13:30")]);
    }
    #[test]
    fn daily_today_if_not_passed_and_end_date() {
        let plan = Plan { repeat: Repeat::Daily, start: "2026-11-05".into(), end: "06/11/2026".into(), ..Default::default() };
        let runs = plan.next_runs(at("2026-11-05 08:00"), 5);
        assert_eq!(runs, vec![at("2026-11-05 13:30"), at("2026-11-06 13:30")]);
    }
    #[test]
    fn monthly_skips_short_months_and_once() {
        let plan = Plan { repeat: Repeat::Monthly, day_of_month: 31, start: "2026-11-01".into(), ..Default::default() };
        let runs = plan.next_runs(at("2026-11-05 08:00"), 2);
        assert_eq!(runs, vec![at("2026-12-31 13:30"), at("2027-01-31 13:30")]);
        let once = Plan { repeat: Repeat::Once, start: "2026-11-10".into(), ..Default::default() };
        assert_eq!(once.next_runs(at("2026-11-05 08:00"), 3), vec![at("2026-11-10 13:30")]);
        assert!(once.next_runs(at("2026-11-11 08:00"), 3).is_empty());
    }
    #[test]
    fn task_xml_is_system_and_catches_up() {
        let plan = Plan { start: "2026-11-05".into(), days: [true, false, true, false, false, false, false], ..Default::default() };
        let xml = scan_task_xml(&plan, Path::new("C:\\Program Files\\TracciaDoc\\TracciaDoc.exe")).unwrap();
        assert!(xml.contains("<UserId>S-1-5-18</UserId>"));
        assert!(xml.contains("<StartWhenAvailable>true</StartWhenAvailable>"));
        assert!(xml.contains("<Monday /><Wednesday />"));
        assert!(xml.contains("<StartBoundary>2026-11-05T13:30:00</StartBoundary>"));
        assert!(xml.contains("<Arguments>--scheduled</Arguments>"));
        assert!(xml.contains("<WorkingDirectory>C:\\Program Files\\TracciaDoc</WorkingDirectory>"));
    }
    #[test]
    fn invalid_plans() {
        let mut p = Plan { days: [false; 7], ..Default::default() };
        assert!(p.validate().is_err());
        p.days[0] = true;
        p.end = "2000-01-01".into();
        assert!(p.validate().is_err());
    }
}
