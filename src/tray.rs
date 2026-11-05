//! `--tray`: icona nell'area di notifica, avviata all'accesso. Processo leggero: niente GUI,
//! GPU, OCR o modelli; legge lo stato delle scansioni pianificate ogni pochi secondi.
#![cfg(windows)]
use crate::{diag, install, sched, update};
use chrono::Local;
use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject},
    System::{LibraryLoader::GetModuleHandleW, Threading::CreateMutexW},
    UI::{
        Shell::{NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW},
        WindowsAndMessaging::*,
    },
};

const WM_TRAY: u32 = WM_APP + 1;
const TIMER: usize = 1;
const TICK_MS: u32 = 3000;
/// Indici delle icone: pronto, scansione, documenti trovati, aggiornamento, errore.
const READY: usize = 0;
const SCAN: usize = 1;
const FOUND: usize = 2;
const UPDATE: usize = 3;
const ERROR: usize = 4;
const DOTS: [Option<(u8, u8, u8)>; 5] = [None, Some((34, 211, 176)), Some((240, 160, 64)), Some((74, 163, 255)), Some((229, 72, 77))];

#[derive(Clone, Copy)]
enum Cmd {
    Open = 10,
    Scan,
    Plan,
    Folder,
    Update,
    CancelShutdown,
    Exit,
}

type Check = Arc<Mutex<Option<(bool, Result<Option<update::Release>, String>)>>>;
struct Tray {
    hwnd: HWND,
    icons: [HICON; 5],
    shown: usize,
    tip: String,
    taskbar_created: u32,
    /// Ultima voce dello storico già notificata.
    seen: Option<String>,
    was_scanning: bool,
    shutdown_notified: i64,
    release: Option<update::Release>,
    check: Check,
    last_check: Option<Instant>,
    exe_mtime: Option<SystemTime>,
    mutex: isize,
}
thread_local!(static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) });

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn fill(dst: &mut [u16], s: &str) {
    let n = dst.len() - 1;
    for (i, u) in s.encode_utf16().take(n).chain(std::iter::repeat(0)).take(n + 1).enumerate() {
        dst[i] = u;
    }
}
/// Icona dell'app con il pallino di stato in basso a destra, disegnata in memoria.
fn make_icon(size: u32, dot: Option<(u8, u8, u8)>) -> HICON {
    let base = image::load_from_memory(include_bytes!("../assets/ui/app-icon-dark-256.png")).expect("Icona incorporata").to_rgba8();
    let mut img = image::imageops::resize(&base, size, size, image::imageops::FilterType::Lanczos3);
    if let Some((r, g, b)) = dot {
        let s = size as f32;
        let (cx, cy, rad) = (s * 0.74, s * 0.74, s * 0.24);
        for y in 0..size {
            for x in 0..size {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                if d <= rad + 1.2 {
                    let px = if d <= rad { image::Rgba([r, g, b, 255]) } else { image::Rgba([10, 12, 16, 255]) };
                    img.put_pixel(x, y, px);
                }
            }
        }
    }
    unsafe {
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let color = CreateDIBSection(std::ptr::null_mut(), &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if color.is_null() || bits.is_null() {
            return std::ptr::null_mut();
        }
        let out = std::slice::from_raw_parts_mut(bits as *mut u8, (size * size * 4) as usize);
        for (i, p) in img.pixels().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
        let mask_bits = vec![0u8; (size.div_ceil(16) * 2 * size) as usize];
        let mask = CreateBitmap(size as i32, size as i32, 1, 1, mask_bits.as_ptr() as _);
        let ii = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
        let icon = CreateIconIndirect(&ii);
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }
}
fn nid(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW { cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd, uID: 1, ..Default::default() }
}
fn add_icon(t: &Tray) {
    let mut n = nid(t.hwnd);
    n.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    n.uCallbackMessage = WM_TRAY;
    n.hIcon = t.icons[t.shown];
    fill(&mut n.szTip, &t.tip);
    unsafe { Shell_NotifyIconW(NIM_ADD, &n) };
}
fn set_icon(t: &mut Tray, which: usize, tip: String) {
    if which == t.shown && tip == t.tip {
        return;
    }
    t.shown = which;
    t.tip = tip;
    let mut n = nid(t.hwnd);
    n.uFlags = NIF_ICON | NIF_TIP;
    n.hIcon = t.icons[which];
    fill(&mut n.szTip, &t.tip);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &n) };
}
fn balloon(t: &Tray, title: &str, text: &str, warn: bool) {
    let mut n = nid(t.hwnd);
    n.uFlags = NIF_INFO;
    n.dwInfoFlags = if warn { NIIF_WARNING } else { NIIF_INFO };
    fill(&mut n.szInfoTitle, title);
    fill(&mut n.szInfo, text);
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &n) };
}
fn exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| install::installed_exe())
}
fn spawn(args: &[&str]) {
    let _ = std::process::Command::new(exe()).args(args).spawn();
}
fn next_run_text() -> String {
    match sched::load_config().plan {
        Some(plan) => plan.next_runs(Local::now().naive_local(), 1).first().map_or("nessuna in calendario".into(), |t| sched::when_text(*t)),
        None => "non pianificata".into(),
    }
}
fn start_check(t: &mut Tray, manual: bool) {
    t.last_check = Some(Instant::now());
    let slot = t.check.clone();
    std::thread::spawn(move || {
        let r = update::check();
        *slot.lock().unwrap() = Some((manual, r));
    });
}
fn tick(t: &mut Tray) {
    // Esito del controllo aggiornamenti in sottofondo.
    let result = t.check.lock().unwrap().take();
    if let Some((manual, r)) = result {
        match r {
            Ok(Some(rel)) => {
                if t.release.as_ref().map(|x| &x.version) != Some(&rel.version) {
                    let auto = sched::load_config().update == sched::UpdateMode::Auto && install::running_installed();
                    balloon(t, &format!("TracciaDoc {} disponibile", rel.version), if auto { "Si installa da solo quando non c'è una scansione in corso. Clic destro sull'icona per aggiornare ora." } else { "Clic destro sull'icona > Aggiorna." }, false);
                }
                t.release = Some(rel);
            }
            Ok(None) => {
                t.release = None;
                if manual {
                    balloon(t, "TracciaDoc è aggiornato", &format!("Versione {} (la più recente).", env!("CARGO_PKG_VERSION")), false);
                }
            }
            Err(e) if manual => balloon(t, "Controllo aggiornamenti non riuscito", &e, true),
            Err(e) => diag::warn(format!("Controllo aggiornamenti: {e}")),
        }
    }
    let cfg = sched::load_config();
    if cfg.update != sched::UpdateMode::Off && t.last_check.is_none_or(|c| c.elapsed() > Duration::from_secs(24 * 3600)) {
        start_check(t, false);
    }
    // EXE installato sostituito da un aggiornamento: riparte con la versione nuova.
    if install::running_installed() {
        let m = std::fs::metadata(install::installed_exe()).and_then(|m| m.modified()).ok();
        if t.exe_mtime.is_some() && m.is_some() && m != t.exe_mtime {
            restart(t);
            return;
        }
    }
    let status = sched::load_status();
    let scanning = sched::scan_running();
    let now = chrono::Utc::now().timestamp();
    if scanning && !t.was_scanning && status.state == "scanning" {
        let off = cfg.plan.as_ref().is_some_and(|p| p.shutdown);
        balloon(t, "Scansione pianificata in corso", if off { "TracciaDoc controlla i dischi. Al termine salva il verbale e spegne il PC (avviso di 60 secondi)." } else { "TracciaDoc controlla i dischi e salva il verbale al termine." }, false);
    }
    t.was_scanning = scanning;
    if status.shutdown_at > now && status.shutdown_at != t.shutdown_notified {
        t.shutdown_notified = status.shutdown_at;
        balloon(t, &format!("Il PC si spegne tra {} secondi", status.shutdown_at - now), "Scansione completata e verbale salvato. Per annullare: clic destro sull'icona TracciaDoc > Annulla spegnimento.", true);
    }
    let latest = status.history.first().cloned();
    if let Some(e) = &latest {
        let key = format!("{}{}", e.when, e.id);
        match &t.seen {
            None => t.seen = Some(key),
            Some(k) if *k != key => {
                t.seen = Some(key);
                match e.outcome.as_str() {
                    "found" => balloon(t, &format!("Trovati {} possibili documenti", e.findings), "Apri TracciaDoc per rivederli ed eliminarli: niente viene cancellato in automatico.", true),
                    "clean" => balloon(t, "Scansione completata", "Nessun documento trovato. Verbale salvato.", false),
                    "incomplete" => balloon(t, "Scansione incompleta", "Alcuni file non sono stati analizzati: vedi il verbale.", false),
                    _ => balloon(t, "Scansione pianificata non riuscita", &e.note, true),
                }
            }
            _ => {}
        }
    }
    let found_unseen = latest.as_ref().is_some_and(|e| e.outcome == "found" && !sched::seen(&e.id));
    // Rosso solo per un errore vero: "incompleta" è l'esito normale su C: e resta nel tooltip.
    let failed = latest.as_ref().is_some_and(|e| e.outcome == "error");
    let incomplete = latest.as_ref().is_some_and(|e| e.outcome == "incomplete");
    let version = env!("CARGO_PKG_VERSION");
    let (which, tip) = if scanning {
        (SCAN, if status.state == "scanning" { format!("TracciaDoc · scansione in corso {}%", status.percent) } else { "TracciaDoc · scansione in corso".into() })
    } else if failed {
        (ERROR, "TracciaDoc · ultima scansione non riuscita".into())
    } else if found_unseen {
        (FOUND, format!("TracciaDoc · {} documenti da rivedere", latest.as_ref().map_or(0, |e| e.findings)))
    } else if let Some(rel) = &t.release {
        (UPDATE, format!("TracciaDoc {version} · disponibile {}", rel.version))
    } else {
        let last = if incomplete { " · ultima incompleta" } else { "" };
        (READY, format!("TracciaDoc {version}{last} · prossima scansione: {}", next_run_text()))
    };
    set_icon(t, which, tip);
}
fn restart(t: &mut Tray) {
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &nid(t.hwnd));
        CloseHandle(t.mutex as _);
    }
    spawn(&["--tray"]);
    unsafe { PostQuitMessage(0) };
}
fn menu(t: &mut Tray) {
    let status = sched::load_status();
    let pending = status.shutdown_at > chrono::Utc::now().timestamp();
    unsafe {
        let m = CreatePopupMenu();
        let item = |flags: u32, id: usize, text: &str| {
            let w = wide(text);
            AppendMenuW(m, flags, id, w.as_ptr());
        };
        item(MF_STRING | MF_GRAYED, 0, &format!("TracciaDoc {} · prossima: {}", env!("CARGO_PKG_VERSION"), next_run_text()));
        if pending {
            item(MF_STRING, Cmd::CancelShutdown as usize, "Annulla spegnimento");
        }
        item(MF_SEPARATOR, 0, "");
        item(MF_STRING, Cmd::Open as usize, "Apri TracciaDoc");
        item(MF_STRING, Cmd::Scan as usize, "Scansiona ora...");
        item(MF_STRING, Cmd::Plan as usize, "Pianificazione...");
        item(MF_SEPARATOR, 0, "");
        item(MF_STRING, Cmd::Folder as usize, "Apri cartella verbali");
        let upd = match &t.release {
            Some(r) => format!("Aggiorna a {}", r.version),
            None => "Cerca aggiornamenti".into(),
        };
        item(MF_STRING, Cmd::Update as usize, &upd);
        item(MF_SEPARATOR, 0, "");
        item(MF_STRING, Cmd::Exit as usize, "Esci (le scansioni pianificate restano attive)");
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(t.hwnd);
        let cmd = TrackPopupMenu(m, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, t.hwnd, std::ptr::null()) as usize;
        DestroyMenu(m);
        command(t, cmd);
    }
}
fn command(t: &mut Tray, cmd: usize) {
    match cmd {
        x if x == Cmd::Open as usize || x == Cmd::Scan as usize => spawn(&[]),
        x if x == Cmd::Plan as usize => spawn(&["--pianificazione"]),
        x if x == Cmd::Folder as usize => {
            let dir = sched::verbali_dir();
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::process::Command::new(PathBuf::from(std::env::var_os("WINDIR").unwrap_or_default()).join("explorer.exe")).arg(dir).spawn();
        }
        x if x == Cmd::Update as usize => {
            if t.release.is_some() && install::is_installed() {
                // L'aggiornamento scrive in Program Files: richiesta UAC, poi l'icona riparte da sola.
                std::thread::spawn(|| {
                    if let Err(e) = install::run_elevated(&install::installed_exe(), "--update") {
                        diag::warn(format!("Aggiornamento manuale: {e}"));
                    }
                });
            } else if t.release.is_some() {
                let _ = std::process::Command::new(PathBuf::from(std::env::var_os("WINDIR").unwrap_or_default()).join("explorer.exe")).arg("https://github.com/realfulvio/tracciadoc/releases/latest").spawn();
            } else {
                start_check(t, true);
            }
        }
        x if x == Cmd::CancelShutdown as usize => {
            sched::cancel_shutdown();
            balloon(t, "Spegnimento annullato", "Il PC resta acceso.", false);
        }
        x if x == Cmd::Exit as usize => unsafe {
            Shell_NotifyIconW(NIM_DELETE, &nid(t.hwnd));
            PostQuitMessage(0);
        },
        _ => {}
    }
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let handled = TRAY.with(|cell| {
        let Ok(mut guard) = cell.try_borrow_mut() else { return false };
        let Some(t) = guard.as_mut() else { return false };
        match msg {
            WM_TRAY => {
                match lp as u32 {
                    WM_RBUTTONUP | WM_CONTEXTMENU => menu(t),
                    WM_LBUTTONDBLCLK => spawn(&[]),
                    _ => {}
                }
                true
            }
            WM_TIMER if wp == TIMER => {
                tick(t);
                true
            }
            m if m == t.taskbar_created => {
                // Explorer riavviato: l'icona va aggiunta di nuovo.
                add_icon(t);
                true
            }
            _ => false,
        }
    });
    if handled { 0 } else { unsafe { DefWindowProcW(hwnd, msg, wp, lp) } }
}

pub fn run() -> Result<(), String> {
    unsafe {
        let name = wide("Local\\TracciaDocTray");
        let mutex = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return Ok(());
        }
        SetProcessDPIAware();
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide("TracciaDocTray");
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: hinst, lpszClassName: class.as_ptr(), ..std::mem::zeroed() };
        RegisterClassW(&wc);
        let title = wide("TracciaDoc");
        let hwnd = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null());
        if hwnd.is_null() {
            return Err("Finestra dell'icona non creata".into());
        }
        let size = GetSystemMetrics(SM_CXSMICON).clamp(16, 64) as u32;
        let icons = std::array::from_fn(|i| make_icon(size, DOTS[i]));
        let tray = Tray {
            hwnd,
            icons,
            shown: READY,
            tip: format!("TracciaDoc {}", env!("CARGO_PKG_VERSION")),
            taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            seen: None,
            was_scanning: false,
            shutdown_notified: 0,
            release: None,
            check: Arc::new(Mutex::new(None)),
            // Primo controllo aggiornamenti dopo un minuto, non all'accesso.
            last_check: Instant::now().checked_sub(Duration::from_secs(24 * 3600 - 60)),
            exe_mtime: std::fs::metadata(install::installed_exe()).and_then(|m| m.modified()).ok(),
            mutex: mutex as isize,
        };
        add_icon(&tray);
        TRAY.with(|c| *c.borrow_mut() = Some(tray));
        TRAY.with(|c| {
            if let Some(t) = c.borrow_mut().as_mut() {
                tick(t);
            }
        });
        SetTimer(hwnd, TIMER, TICK_MS, None);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        TRAY.with(|c| {
            if let Some(t) = c.borrow_mut().take() {
                Shell_NotifyIconW(NIM_DELETE, &nid(t.hwnd));
                for i in t.icons {
                    DestroyIcon(i);
                }
            }
        });
    }
    Ok(())
}
