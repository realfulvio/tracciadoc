use crate::{diag, model::*};
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
        ignored_type: 0,
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
                return Err(crate::parser::INTERRUPTED.into());
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
    let value = serde_json::json!({"event":event,"id":run.id,"time":now(),"version":env!("CARGO_PKG_VERSION"),"rules":RULES,"deep":run.settings.deep,"encountered":run.encountered,"analyzed":run.analyzed,"partial_files":run.partial_files,"skipped":run.skipped,"ignored_type":run.ignored_type,"findings":run.findings.len(),"incomplete":run.incomplete(),"issues":run.issues,"seconds":run.elapsed_secs});
    serde_json::to_writer(&mut file, &value)?;
    writeln!(file)?;
    file.sync_data()
}
struct Outcome {
    path: PathBuf,
    result: Result<Analysis, String>,
    notes: Vec<&'static str>,
    peak: u64,
    roundtrip_ms: f64,
    /// The worker could not be started: stop dispatching.
    fatal: bool,
}
/// One isolated parser process per thread, reused across files and respawned after a crash.
fn worker_loop(
    settings: Settings,
    jobs: Arc<Mutex<mpsc::Receiver<Request>>>,
    out: mpsc::Sender<Outcome>,
    stop: Arc<AtomicBool>,
    fallback: Arc<AtomicBool>,
) {
    let mut worker: Option<(Worker, bool)> = None;
    loop {
        let Ok(request) = jobs.lock().unwrap().recv() else {
            break;
        };
        let roundtrip = Instant::now();
        let mut o = Outcome {
            path: request.path.clone(),
            result: Err(crate::parser::INTERRUPTED.into()),
            notes: vec![],
            peak: 0,
            roundtrip_ms: 0.,
            fatal: false,
        };
        if !stop.load(Ordering::Relaxed) {
            // After the first crash every worker moves to the generic scalar product.
            let cpu_only = settings.cpu_only || fallback.load(Ordering::Relaxed);
            if worker.as_ref().is_some_and(|(_, c)| *c != cpu_only) {
                worker = None;
            }
            if worker.is_none() {
                let mut s = settings.clone();
                s.cpu_only = cpu_only;
                match Worker::spawn(&s) {
                    Ok(w) => worker = Some((w, cpu_only)),
                    Err(e) => {
                        o.result = Err(e.to_string());
                        o.fatal = true;
                    }
                }
            }
            if let Some((w, _)) = worker.as_mut() {
                o.result = w.analyze(&request, &stop, &mut o.peak);
                if o
                    .result
                    .as_ref()
                    .is_err_and(|e| e.starts_with("Parser terminato"))
                    && !settings.cpu_only
                    && !fallback.swap(true, Ordering::Relaxed)
                    && !stop.load(Ordering::Relaxed)
                {
                    worker = None;
                    o.notes.push("Arresto parser: fallback al prodotto scalare CPU generico e nuova analisi del file");
                    let mut safe = settings.clone();
                    safe.cpu_only = true;
                    o.result = match Worker::spawn(&safe) {
                        Ok(mut w) => {
                            let r = w.analyze(&request, &stop, &mut o.peak);
                            worker = Some((w, true));
                            r
                        }
                        Err(e) => Err(e.to_string()),
                    };
                }
                if o.result.is_err() {
                    worker = None;
                }
            }
        }
        o.roundtrip_ms = roundtrip.elapsed().as_secs_f64() * 1000.;
        if out.send(o).is_err() {
            break;
        }
    }
}
/// Applies one file result to the run; true when dispatching must stop.
fn collect(state: &mut Run, o: Outcome, stop: &AtomicBool) -> bool {
    state.peak_worker_bytes = state.peak_worker_bytes.max(o.peak);
    let shown = diag::shown(&o.path);
    for note in o.notes {
        if !crate::parser::is_quiet(&note) {
            diag::warn_file(&note, format!("{shown} · {note}"));
        }
        state.issue(note);
    }
    match o.result {
        Ok(mut result) => {
            result.timing.roundtrip_ms = o.roundtrip_ms;
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
                if !crate::parser::is_quiet(&issue) {
                    diag::warn_file(&issue, format!("{shown} · {issue}"));
                }
                state.issue(issue);
            }
            if !result.reasons.is_empty() {
                diag::info(format!("SEGNALATO {shown} ({})", result.format));
            }
            if !result.reasons.is_empty() && state.findings.len() < 5000 {
                let meta = std::fs::metadata(&o.path).ok();
                state.findings.push(Finding {
                    size: meta.as_ref().map_or(0, |m| m.len()),
                    modified: meta.and_then(|m| m.modified().ok()),
                    path: o.path,
                    format: result.format,
                    reasons: result.reasons,
                    review: Review::Pending,
                    ..Default::default()
                });
            }
        }
        Err(e) => {
            if e == crate::parser::UNSUPPORTED {
                // Firma non di interesse dopo lo sniff: come i file ignorati per tipo, non e un problema.
                state.ignored_type += 1;
            } else if e == crate::parser::INTERRUPTED {
                state.skipped += 1;
            } else {
                state.skipped += 1;
                diag::err_file(&e, format!("{shown} · {e}"));
                state.issue(e);
            }
            if stop.load(Ordering::Relaxed) {
                state.interrupted = true;
                return true;
            }
        }
    }
    if state.findings.len() >= 5000 {
        if !state.issues.contains_key("Limite 5000 segnalazioni raggiunto") {
            state.issue("Limite 5000 segnalazioni raggiunto");
        }
        return true;
    }
    o.fatal
}
pub fn run(shared: Shared, stop: Arc<AtomicBool>) {
    let start = Instant::now();
    let mut state = shared.lock().unwrap().run.clone().unwrap();
    diag::reset_caps();
    diag::info(format!(
        "AVVIO verifica · aree: {} · worker {}",
        state
            .roots
            .iter()
            .map(|r| diag::shown(r))
            .collect::<Vec<_>>()
            .join(", "),
        state.settings.workers
    ));
    if audit(&state, "start").is_err() {
        state.audit_error = true;
        state.issue(diag::noted("Storico locale non scrivibile o pieno"));
    }
    let workers = state.settings.workers.max(1);
    let (job_tx, job_rx) = mpsc::sync_channel::<Request>(workers);
    let job_rx = Arc::new(Mutex::new(job_rx));
    let (out_tx, out_rx) = mpsc::channel::<Outcome>();
    let fallback = Arc::new(AtomicBool::new(false));
    let pool: Vec<_> = (0..workers)
        .map(|_| {
            let (settings, jobs, out, stop, fallback) = (
                state.settings.clone(),
                job_rx.clone(),
                out_tx.clone(),
                stop.clone(),
                fallback.clone(),
            );
            thread::spawn(move || worker_loop(settings, jobs, out, stop, fallback))
        })
        .collect();
    drop(out_tx);
    // Halted: no further files are dispatched (limits, fatal worker error, stop).
    let mut halted = false;
    let mut published = Instant::now();
    let roots = state.roots.clone();
    let mut scanned_roots: Vec<PathBuf> = vec![];
    'roots: for root in roots {
        diag::info(format!("Area: {}", diag::shown(&root)));
        if is_network(&root) {
            state.issue(diag::noted("Percorso di rete escluso: ambito solo locale"));
            continue;
        }
        if !root.exists() {
            state.issue(diag::noted("Percorso selezionato non disponibile o accesso negato"));
            continue;
        }
        // An explicitly selected volume/mount root is allowed; descendant reparses are not followed.
        let root_reparse =
            std::fs::symlink_metadata(&root).is_ok_and(|m| m.file_attributes() & 0x400 != 0);
        if root_reparse && !is_volume_mount(&root) {
            state.issue(diag::noted("Radice collegamento/junction esclusa: selezionare la destinazione locale"));
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
                state.issue(diag::noted("Limite file raggiunto"));
                break 'roots;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    state.issue(diag::noted("Enumerazione: accesso negato o voce non disponibile"));
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
                state.issue(diag::noted("Collegamento, junction o volume montato non attraversato: selezionare la destinazione separatamente"));
                continue;
            }
            if entry.file_type().is_dir() {
                continue;
            }
            state.encountered += 1;
            if state.encountered.is_multiple_of(2000) {
                diag::info(format!(
                    "Avanzamento · visti {} · analizzati {} · segnalati {}",
                    state.encountered,
                    state.analyzed,
                    state.findings.len()
                ));
            }
            let metadata = match std::fs::symlink_metadata(path) {
                Ok(m) => m,
                Err(_) => {
                    state.skipped += 1;
                    state.issue(diag::noted("File: metadati non leggibili"));
                    continue;
                }
            };
            if !crate::parser::worth_opening(
                path,
                metadata.len(),
                state.settings.max_mb * 1024 * 1024,
            ) {
                state.ignored_type += 1;
                continue;
            }
            if metadata.file_attributes() & (0x400 | 0x1000 | 0x400000 | 0x4000) != 0 {
                state.skipped += 1;
                state.issue(diag::noted("File reparse, offline, cloud recall o cifrato EFS non aperto"));
                continue;
            }
            if published.elapsed() >= Duration::from_millis(100) || state.encountered == 1 {
                let mut p = shared.lock().unwrap();
                p.current = path.to_string_lossy().into_owned();
                p.phase = format!("Analisi testo e OCR locale ({workers} worker)");
                p.run = Some(state.clone());
                published = Instant::now();
            }
            let mut request = Request {
                path: path.to_path_buf(),
                settings: state.settings.clone(),
            };
            // Dispatch while draining results, so the bounded queue never deadlocks.
            loop {
                match job_tx.try_send(request) {
                    Ok(()) => break,
                    Err(mpsc::TrySendError::Full(r)) => {
                        request = r;
                        if let Ok(o) = out_rx.recv_timeout(Duration::from_millis(50)) {
                            halted |= collect(&mut state, o, &stop);
                        }
                        if stop.load(Ordering::Relaxed) {
                            state.interrupted = true;
                            break 'roots;
                        }
                        if halted {
                            break 'roots;
                        }
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => break 'roots,
                }
            }
            while let Ok(o) = out_rx.try_recv() {
                halted |= collect(&mut state, o, &stop);
            }
            if halted {
                break 'roots;
            }
            state.elapsed_secs = start.elapsed().as_secs_f64();
            if published.elapsed() >= Duration::from_millis(100) {
                shared.lock().unwrap().run = Some(state.clone());
                published = Instant::now();
            }
        }
    }
    drop(job_tx);
    for o in out_rx {
        collect(&mut state, o, &stop);
        state.elapsed_secs = start.elapsed().as_secs_f64();
        if published.elapsed() >= Duration::from_millis(100) {
            shared.lock().unwrap().run = Some(state.clone());
            published = Instant::now();
        }
    }
    for t in pool {
        let _ = t.join();
    }
    // Parallel workers complete out of order: stable output independent of scheduling.
    state.findings.sort_by(|a, b| a.path.cmp(&b.path));
    state.interrupted |= stop.load(Ordering::Relaxed);
    state.finished = true;
    state.end = now();
    state.elapsed_secs = start.elapsed().as_secs_f64();
    if audit(&state, "end").is_err() {
        state.audit_error = true;
        state.issue(diag::noted("Storico locale non scrivibile o pieno"));
    }
    let summary = format!(
        "FINE verifica · {} · visti {} · analizzati {} · parziali {} · saltati {} · segnalati {} · {:.1} s",
        if state.interrupted { "INTERROTTA" } else { "completata" },
        state.encountered,
        state.analyzed,
        state.partial_files,
        state.skipped,
        state.findings.len(),
        state.elapsed_secs
    );
    if state.interrupted || !state.issues.is_empty() {
        diag::warn(summary);
    } else {
        diag::ok(summary);
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
/// Physical cores (not SMT threads): OCR workers are CPU-bound.
pub fn physical_cores() -> usize {
    use windows_sys::Win32::System::SystemInformation::{
        GetLogicalProcessorInformation, RelationProcessorCore,
        SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
    };
    let logical = thread::available_parallelism().map_or(1, |n| n.get());
    let mut len = 0u32;
    unsafe {
        GetLogicalProcessorInformation(std::ptr::null_mut(), &mut len);
        let n = len as usize / std::mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
        let mut info: Vec<SYSTEM_LOGICAL_PROCESSOR_INFORMATION> = Vec::with_capacity(n);
        if n == 0 || GetLogicalProcessorInformation(info.as_mut_ptr(), &mut len) == 0 {
            return logical;
        }
        info.set_len(n);
        let cores = info
            .iter()
            .filter(|i| i.Relationship == RelationProcessorCore)
            .count();
        if cores == 0 { logical } else { cores }
    }
}
/// Default OCR workers: physical cores minus one for the coordinator and GUI.
pub fn default_workers() -> usize {
    physical_cores().saturating_sub(1).max(1)
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
