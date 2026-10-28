use crate::model::*;
use chrono::Local;
use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    os::windows::{fs::MetadataExt, io::AsRawHandle, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        JobObjects::*,
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
    },
};

pub type Shared = Arc<Mutex<Progress>>;
#[derive(Default)]
pub struct Progress {
    pub run: Option<Run>,
    pub current: String,
    pub phase: String,
}
pub fn now() -> String {
    Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}
pub fn new_run(
    roots: Vec<PathBuf>,
    mut settings: Settings,
    operator: String,
    reference: String,
) -> Run {
    // Every NEW scan is complete. Keep the field only to accurately represent historical records.
    settings.deep = true;
    Run {
        id: Local::now().format("%Y%m%d-%H%M%S-%f").to_string(),
        computer: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC non identificato".into()),
        operator,
        reference,
        roots,
        settings,
        start: now(),
        end: String::new(),
        elapsed_secs: 0.,
        encountered: 0,
        analyzed: 0,
        partial_files: 0,
        skipped: 0,
        pages_ocr: 0,
        ocr_seconds: 0.,
        ocr_init_ms: 0.,
        issues: Default::default(),
        findings: vec![],
        interrupted: false,
        finished: false,
        backend: "CPU; nessuna cache; nessun provider GPU".into(),
        peak_worker_bytes: 0,
        audit_error: false,
        previous: None,
        timing: Default::default(),
        timing_by_format: Default::default(),
    }
}
struct Worker {
    child: Child,
    input: Option<ChildStdin>,
    rx: mpsc::Receiver<Option<Analysis>>,
    reader: Option<thread::JoinHandle<()>>,
    job: HANDLE,
}
impl Worker {
    fn spawn(settings: &Settings) -> Result<Self, String> {
        let mut child = Command::new(std::env::current_exe().map_err(|_| "EXE non individuato")?)
            .arg("--worker")
            .current_dir(crate::parser::runtime().join("ocr"))
            .env("OMP_THREAD_LIMIT", "1")
            .env("OMP_NUM_THREADS", "1")
            .env("DOTPRODUCT", cpu_method(settings.cpu_only))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|_| "Avvio parser isolato fallito")?;
        let job = unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Isolamento Job Windows non disponibile".into());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags =
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            info.ProcessMemoryLimit = (settings.memory_mb * 1024 * 1024) as usize;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) == 0
            {
                CloseHandle(job);
                let _ = child.kill();
                let _ = child.wait();
                return Err("Limiti Job Windows non applicabili".into());
            }
            job
        };
        let input = child.stdin.take();
        let output = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        let reader = thread::spawn(move || {
            let mut output = BufReader::new(output);
            loop {
                let mut line = String::new();
                use std::io::Read;
                match output.by_ref().take(100_001).read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    _ => {
                        if line.len() > 100_000
                            || tx.try_send(serde_json::from_str(&line).ok()).is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self {
            child,
            input,
            rx,
            reader: Some(reader),
            job,
        })
    }
    fn analyze(
        &mut self,
        req: &Request,
        stop: &AtomicBool,
        peak: &mut u64,
    ) -> Result<Analysis, String> {
        let input = self.input.as_mut().unwrap();
        serde_json::to_writer(&mut *input, req).map_err(|_| "Parser terminato")?;
        writeln!(input)
            .and_then(|_| input.flush())
            .map_err(|_| "Parser terminato")?;
        let start = Instant::now();
        loop {
            if stop.load(Ordering::Relaxed) {
                return Err("Verifica interrotta".into());
            }
            unsafe {
                let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
                counters.cb = std::mem::size_of_val(&counters) as u32;
                if GetProcessMemoryInfo(
                    self.child.as_raw_handle() as HANDLE,
                    &mut counters,
                    counters.cb,
                ) != 0
                {
                    *peak = (*peak).max(counters.PeakWorkingSetSize as u64);
                }
            }
            match self.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Some(a)) => return Ok(a),
                Ok(None) => return Err("Risposta parser non valida".into()),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Parser terminato: crash o limite memoria".into());
                }
                _ => {}
            }
            if start.elapsed().as_secs() >= req.settings.timeout_secs {
                return Err("Timeout parser/OCR; processo recuperato".into());
            }
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        unsafe {
            CloseHandle(self.job);
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
pub fn state_dir() -> PathBuf {
    PathBuf::from(
        std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| std::env::temp_dir().into_os_string()),
    )
    .join("TracciaDoc")
}
pub fn audit(run: &Run, event: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(state_dir())?;
    let path = state_dir().join("audit.jsonl");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 5 * 1024 * 1024) {
        return Err(std::io::Error::other(
            "Storico pieno: rimuovere o archiviare audit.jsonl",
        ));
    }
    let mut file = OpenOptions::new().append(true).create(true).open(path)?;
    // No paths, operator names, document contents or identifiers in persistent audit.
    let value = serde_json::json!({"event":event,"id":run.id,"time":now(),"version":env!("CARGO_PKG_VERSION"),"rules":RULES,"deep":run.settings.deep,"encountered":run.encountered,"analyzed":run.analyzed,"partial_files":run.partial_files,"skipped":run.skipped,"findings":run.findings.len(),"incomplete":run.incomplete(),"issues":run.issues,"seconds":run.elapsed_secs});
    serde_json::to_writer(&mut file, &value)?;
    writeln!(file)?;
    file.sync_data()
}
pub fn run(shared: Shared, stop: Arc<AtomicBool>) {
    let start = Instant::now();
    let mut state = shared.lock().unwrap().run.clone().unwrap();
    if audit(&state, "start").is_err() {
        state.audit_error = true;
        state.issue("Storico locale non scrivibile o pieno");
    }
    let mut worker = None;
    let mut fallback_cpu = false;
    let mut published = Instant::now();
    let roots = state.roots.clone();
    let mut scanned_roots: Vec<PathBuf> = vec![];
    'roots: for root in roots {
        if is_network(&root) {
            state.issue("Percorso di rete escluso: ambito solo locale");
            continue;
        }
        if !root.exists() {
            state.issue("Percorso selezionato non disponibile o accesso negato");
            continue;
        }
        // An explicitly selected volume/mount root is allowed; descendant reparses are not followed.
        let root_reparse =
            std::fs::symlink_metadata(&root).is_ok_and(|m| m.file_attributes() & 0x400 != 0);
        if root_reparse && !is_volume_mount(&root) {
            state.issue("Radice collegamento/junction esclusa: selezionare la destinazione locale");
            continue;
        }
        if scanned_roots.iter().any(|r| root.starts_with(r)) && !is_volume_mount(&root) {
            continue;
        }
        scanned_roots.push(root.clone());
        let mut entries = walkdir::WalkDir::new(&root)
            .follow_links(false)
            .max_open(16)
            .into_iter();
        while let Some(entry) = entries.next() {
            if stop.load(Ordering::Relaxed) {
                state.interrupted = true;
                break 'roots;
            }
            if state.encountered >= state.settings.max_entries {
                state.issue("Limite file raggiunto");
                break 'roots;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    state.issue("Enumerazione: accesso negato o voce non disponibile");
                    continue;
                }
            };
            let path = entry.path();
            if entry.depth() > 0
                && std::fs::symlink_metadata(path).is_ok_and(|m| {
                    m.file_attributes() & 0x400 != 0
                        && (m.is_dir() || entry.file_type().is_symlink())
                })
            {
                if entry.file_type().is_dir() {
                    entries.skip_current_dir();
                }
                state.issue("Collegamento, junction o volume montato non attraversato: selezionare la destinazione separatamente");
                continue;
            }
            if entry.file_type().is_dir() {
                continue;
            }
            state.encountered += 1;
            let metadata = match std::fs::symlink_metadata(path) {
                Ok(m) => m,
                Err(_) => {
                    state.skipped += 1;
                    state.issue("File: metadati non leggibili");
                    continue;
                }
            };
            if metadata.file_attributes() & (0x400 | 0x1000 | 0x400000 | 0x4000) != 0 {
                state.skipped += 1;
                state.issue("File reparse, offline, cloud recall o cifrato EFS non aperto");
                continue;
            }
            if published.elapsed() >= Duration::from_millis(100) || state.encountered == 1 {
                let mut p = shared.lock().unwrap();
                p.current = path.to_string_lossy().into_owned();
                p.phase = if state.settings.deep {
                    "Analisi testo e OCR locale"
                } else {
                    "Estrazione preliminare"
                }
                .into();
                p.run = Some(state.clone());
                published = Instant::now();
            }
            if worker.is_none() {
                let mut worker_settings = state.settings.clone();
                worker_settings.cpu_only |= fallback_cpu;
                match Worker::spawn(&worker_settings) {
                    Ok(w) => worker = Some(w),
                    Err(e) => {
                        state.issue(e);
                        state.skipped += 1;
                        break 'roots;
                    }
                }
            }
            let request = Request {
                path: path.to_path_buf(),
                settings: state.settings.clone(),
            };
            let roundtrip = Instant::now();
            let mut result =
                worker
                    .as_mut()
                    .unwrap()
                    .analyze(&request, &stop, &mut state.peak_worker_bytes);
            if result
                .as_ref()
                .is_err_and(|e| e.starts_with("Parser terminato"))
                && !state.settings.cpu_only
                && !fallback_cpu
                && !stop.load(Ordering::Relaxed)
            {
                worker.take();
                fallback_cpu = true;
                state.issue("Arresto parser: fallback al prodotto scalare CPU generico e nuova analisi del file");
                let mut safe = state.settings.clone();
                safe.cpu_only = true;
                result = match Worker::spawn(&safe) {
                    Ok(w) => {
                        worker = Some(w);
                        worker.as_mut().unwrap().analyze(
                            &request,
                            &stop,
                            &mut state.peak_worker_bytes,
                        )
                    }
                    Err(e) => Err(e),
                };
            }
            match result {
                Ok(mut result) => {
                    result.timing.roundtrip_ms = roundtrip.elapsed().as_secs_f64() * 1000.;
                    state.timing.add(&result.timing);
                    state
                        .timing_by_format
                        .entry(result.format.clone())
                        .or_default()
                        .add(&result.timing);
                    state.ocr_seconds += result.ocr_seconds;
                    state.ocr_init_ms += result.ocr_init_ms;
                    if result.pages_ocr > 0 && !state.backend.contains(&result.backend) {
                        if state.pages_ocr == 0 {
                            state.backend = result.backend.clone();
                        } else {
                            state.backend.push_str("; ");
                            state.backend.push_str(&result.backend);
                        }
                    }
                    state.pages_ocr += result.pages_ocr;
                    if result.issues.is_empty() && result.supported {
                        state.analyzed += 1;
                    } else if result.supported {
                        state.partial_files += 1;
                    } else {
                        state.skipped += 1;
                    }
                    for issue in result.issues {
                        state.issue(issue);
                    }
                    if !result.reasons.is_empty() {
                        let meta = std::fs::metadata(path).ok();
                        state.findings.push(Finding {
                            size: meta.as_ref().map_or(0, |m| m.len()),
                            modified: meta.and_then(|m| m.modified().ok()),
                            path: path.to_path_buf(),
                            format: result.format,
                            reasons: result.reasons,
                            review: Review::Pending,
                            ..Default::default()
                        });
                    }
                }
                Err(e) => {
                    state.skipped += 1;
                    state.issue(e);
                    worker.take();
                    if stop.load(Ordering::Relaxed) {
                        state.interrupted = true;
                        break 'roots;
                    }
                }
            }
            state.elapsed_secs = start.elapsed().as_secs_f64();
            if state.findings.len() >= 5000 {
                state.issue("Limite 5000 segnalazioni raggiunto");
                break 'roots;
            }
            if published.elapsed() >= Duration::from_millis(100) {
                shared.lock().unwrap().run = Some(state.clone());
                published = Instant::now();
            }
        }
    }
    drop(worker);
    state.interrupted |= stop.load(Ordering::Relaxed);
    state.finished = true;
    state.end = now();
    state.elapsed_secs = start.elapsed().as_secs_f64();
    if audit(&state, "end").is_err() {
        state.audit_error = true;
        state.issue("Storico locale non scrivibile o pieno");
    }
    let mut p = shared.lock().unwrap();
    p.phase = "Verifica terminata".into();
    p.current.clear();
    p.run = Some(state);
}

pub fn is_network(path: &Path) -> bool {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    let s = path.to_string_lossy();
    if s.starts_with("\\\\") && !s.starts_with("\\\\?\\Volume{") {
        return true;
    }
    let root = if s.len() > 1 && s.as_bytes()[1] == b':' {
        format!("{}\\", &s[..2])
    } else {
        s.into_owned()
    };
    let w: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
    unsafe { GetDriveTypeW(w.as_ptr()) == 4 }
}
pub fn cpu_method(baseline: bool) -> &'static str {
    if baseline {
        "generic"
    } else if std::is_x86_feature_detected!("avx2") {
        "avx2"
    } else if std::is_x86_feature_detected!("sse4.1") {
        "sse"
    } else {
        "generic"
    }
}
fn is_volume_mount(path: &Path) -> bool {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW;
    let s = format!("{}\\", path.to_string_lossy().trim_end_matches('\\'));
    let w: Vec<u16> = s.encode_utf16().chain(Some(0)).collect();
    let mut name = [0u16; 1024];
    unsafe { GetVolumeNameForVolumeMountPointW(w.as_ptr(), name.as_mut_ptr(), 1024) != 0 }
}

pub fn volumes() -> Vec<(PathBuf, String)> {
    use windows_sys::Win32::Storage::FileSystem::*;
    let mut result = vec![];
    unsafe {
        let mut buffer = [0u16; 1024];
        let handle = FindFirstVolumeW(buffer.as_mut_ptr(), 1024);
        if handle as isize == -1 {
            return result;
        }
        loop {
            let len = buffer.iter().position(|b| *b == 0).unwrap_or(0);
            let volume = String::from_utf16_lossy(&buffer[..len]);
            let mut mounts = [0u16; 8192];
            let mut required = 0;
            let ok = GetVolumePathNamesForVolumeNameW(
                buffer.as_ptr(),
                mounts.as_mut_ptr(),
                8192,
                &mut required,
            );
            let mut names: Vec<String> = if ok != 0 {
                mounts
                    .split(|b| *b == 0)
                    .filter(|s| !s.is_empty())
                    .map(String::from_utf16_lossy)
                    .collect()
            } else {
                vec![]
            };
            if names.is_empty() {
                names.push(volume);
            }
            for name in names {
                let w: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
                let kind = match GetDriveTypeW(w.as_ptr()) {
                    2 => "rimovibile",
                    3 => "locale fisso",
                    4 => "rete (esclusa)",
                    5 => "ottico",
                    6 => "RAM",
                    _ => "non disponibile",
                };
                result.push((PathBuf::from(&name), format!("{name}  -  {kind}")));
            }
            if FindNextVolumeW(handle, buffer.as_mut_ptr(), 1024) == 0 {
                break;
            }
        }
        FindVolumeClose(handle);
        let mut drives = [0u16; 4096];
        let n = GetLogicalDriveStringsW(4096, drives.as_mut_ptr());
        if n > 0 && n < 4096 {
            for name in drives[..n as usize]
                .split(|b| *b == 0)
                .filter(|s| !s.is_empty())
                .map(String::from_utf16_lossy)
            {
                if result
                    .iter()
                    .any(|(p, _)| p.to_string_lossy().eq_ignore_ascii_case(&name))
                {
                    continue;
                }
                let w: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
                let kind = match GetDriveTypeW(w.as_ptr()) {
                    2 => "rimovibile",
                    3 => "locale fisso",
                    4 => "rete (esclusa)",
                    5 => "ottico",
                    _ => "non disponibile",
                };
                result.push((PathBuf::from(&name), format!("{name} - {kind}")));
            }
        }
    }
    result
}
