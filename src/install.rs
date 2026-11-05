//! Installazione per tutta la postazione in Program Files, avvio dell'icona all'accesso,
//! voce in "App installate", elevazione (UAC) e sostituzione dei file anche in uso.
use crate::{diag, sched};
use std::path::{Path, PathBuf};

const RUN_KEY: &str = "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const UNINSTALL_KEY: &str = "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\TracciaDoc";

pub fn install_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("ProgramW6432").or_else(|| std::env::var_os("ProgramFiles")).unwrap_or_else(|| "C:\\Program Files".into())).join("TracciaDoc")
}
pub fn installed_exe() -> PathBuf {
    install_dir().join("TracciaDoc.exe")
}
pub fn is_installed() -> bool {
    installed_exe().exists()
}
/// Vero se questo EXE è quello installato (e non una copia portatile).
pub fn running_installed() -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).ok();
    std::env::current_exe().ok().and_then(|e| canon(&e)).is_some_and(|e| Some(e) == canon(&installed_exe()))
}
fn start_menu_link() -> PathBuf {
    PathBuf::from(std::env::var_os("ProgramData").unwrap_or_else(|| "C:\\ProgramData".into())).join("Microsoft\\Windows\\Start Menu\\Programs\\TracciaDoc.lnk")
}

#[cfg(windows)]
pub fn is_admin() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}
#[cfg(not(windows))]
pub fn is_admin() -> bool {
    false
}
/// Rilancia questo EXE come amministratore (richiesta UAC) e attende la fine. Ok(codice di uscita).
#[cfg(windows)]
pub fn run_elevated(exe: &Path, args: &str) -> Result<u32, String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject},
        UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
    };
    let w = |s: &str| -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() };
    let (verb, file, params) = (w("runas"), w(&exe.to_string_lossy()), w(args));
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = 0;
    if unsafe { ShellExecuteExW(&mut info) } == 0 || info.hProcess.is_null() {
        return Err("Autorizzazione di amministratore negata o non disponibile".into());
    }
    let mut code = 1u32;
    unsafe {
        WaitForSingleObject(info.hProcess, INFINITE);
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
    }
    Ok(code)
}
#[cfg(not(windows))]
pub fn run_elevated(_exe: &Path, _args: &str) -> Result<u32, String> {
    Err("Solo Windows".into())
}

/// Copia `src` in `dst` file per file. I file esistenti (anche l'EXE o le DLL in uso)
/// vengono rinominati in .old prima della copia; al primo errore si ripristina tutto.
pub fn replace_tree(src: &Path, dst: &Path) -> Result<(), String> {
    let mut files = vec![];
    for e in walkdir::WalkDir::new(src).into_iter().filter_map(Result::ok) {
        if e.file_type().is_file() {
            files.push(e.path().strip_prefix(src).map_err(|e| e.to_string())?.to_path_buf());
        }
    }
    let mut done: Vec<(PathBuf, bool)> = vec![];
    let result = (|| -> Result<(), String> {
        for rel in &files {
            let target = dst.join(rel);
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            let old = old_name(&target);
            let existed = target.exists();
            if existed {
                let _ = std::fs::remove_file(&old);
                std::fs::rename(&target, &old).map_err(|e| format!("{} non sostituibile: {e}", target.display()))?;
            }
            done.push((target.clone(), existed));
            std::fs::copy(src.join(rel), &target).map_err(|e| format!("{} non copiato: {e}", target.display()))?;
        }
        Ok(())
    })();
    if result.is_err() {
        for (target, existed) in done.iter().rev() {
            let _ = std::fs::remove_file(target);
            if *existed {
                let _ = std::fs::rename(old_name(target), target);
            }
        }
    }
    result
}
fn old_name(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".old");
    PathBuf::from(s)
}
/// Rimuove i .old rimasti da un aggiornamento precedente (quelli ancora in uso restano).
pub fn cleanup_old() {
    for e in walkdir::WalkDir::new(install_dir()).into_iter().filter_map(Result::ok) {
        if e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "old") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// `--install`: copia la cartella del programma, avvio dell'icona all'accesso, attività pianificate.
pub fn install() -> Result<(), String> {
    if !is_admin() {
        return Err("Serve l'amministratore".into());
    }
    let src = std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("Cartella EXE")?.to_path_buf();
    let dst = install_dir();
    if !running_installed() {
        if !src.join("runtime").is_dir() {
            return Err("Manca la cartella runtime accanto all'EXE: avvia l'installazione dalla cartella dello zip".into());
        }
        replace_tree(&src, &dst)?;
        cleanup_old();
    }
    register(&installed_exe())?;
    let cfg = sched::load_config();
    sched::apply_config(&cfg, &installed_exe())?;
    diag::ok(format!("TracciaDoc {} installato in {}", env!("CARGO_PKG_VERSION"), dst.display()));
    Ok(())
}
/// Voci di registro e collegamento nel menu Start, puntati all'EXE installato.
pub fn register(exe: &Path) -> Result<(), String> {
    let exe_s = exe.to_string_lossy().to_string();
    let reg = |key: &str, name: &str, kind: &str, value: &str| sched::run_tool("reg.exe", &["add", key, "/v", name, "/t", kind, "/d", value, "/f"]).map(|_| ());
    reg(RUN_KEY, "TracciaDoc", "REG_SZ", &format!("\"{exe_s}\" --tray"))?;
    reg(UNINSTALL_KEY, "DisplayName", "REG_SZ", "TracciaDoc")?;
    reg(UNINSTALL_KEY, "DisplayVersion", "REG_SZ", env!("CARGO_PKG_VERSION"))?;
    reg(UNINSTALL_KEY, "Publisher", "REG_SZ", "hcok")?;
    reg(UNINSTALL_KEY, "DisplayIcon", "REG_SZ", &exe_s)?;
    reg(UNINSTALL_KEY, "InstallLocation", "REG_SZ", &install_dir().to_string_lossy())?;
    reg(UNINSTALL_KEY, "UninstallString", "REG_SZ", &format!("\"{exe_s}\" --uninstall"))?;
    reg(UNINSTALL_KEY, "NoModify", "REG_DWORD", "1")?;
    reg(UNINSTALL_KEY, "NoRepair", "REG_DWORD", "1")?;
    // Collegamento nel menu Start: facoltativo, un errore non blocca l'installazione.
    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Description='TracciaDoc - verifica documenti';$s.Save()",
        start_menu_link().to_string_lossy().replace('\'', "''"),
        exe_s.replace('\'', "''"),
        install_dir().to_string_lossy().replace('\'', "''")
    );
    if let Err(e) = sched::run_tool("WindowsPowerShell\\v1.0\\powershell.exe", &["-NoProfile", "-NonInteractive", "-Command", &script]) {
        diag::warn(format!("Collegamento nel menu Start non creato: {e}"));
    }
    Ok(())
}
/// `--uninstall`: toglie attività, avvio automatico, voci e cartella del programma.
/// Restano in %ProgramData%\TracciaDoc i verbali e lo storico delle scansioni.
pub fn uninstall() -> Result<(), String> {
    if !is_admin() {
        return Err("Serve l'amministratore".into());
    }
    sched::delete_task(sched::TASK_SCAN);
    sched::delete_task(sched::TASK_UPDATE);
    let _ = sched::run_tool("reg.exe", &["delete", RUN_KEY, "/v", "TracciaDoc", "/f"]);
    let _ = sched::run_tool("reg.exe", &["delete", UNINSTALL_KEY, "/f"]);
    let _ = std::fs::remove_file(start_menu_link());
    // Chiude le icone nell'area di notifica (tutte le sessioni), non questo processo.
    let filter = format!("PID ne {}", std::process::id());
    let _ = sched::run_tool("taskkill.exe", &["/F", "/IM", "TracciaDoc.exe", "/FI", &filter]);
    let dir = install_dir();
    if running_installed() {
        // L'EXE in esecuzione non si cancella da solo: ci pensa cmd dopo qualche secondo.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let cmd = format!("ping -n 4 127.0.0.1 >nul & rmdir /s /q \"{}\"", dir.display());
            let _ = std::process::Command::new(PathBuf::from(std::env::var_os("WINDIR").unwrap_or_default()).join("System32\\cmd.exe"))
                .raw_arg("/c")
                .raw_arg(&cmd)
                .creation_flags(0x0800_0000 | 0x0000_0008)
                .spawn();
        }
    } else {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{} non rimossa: {e}", dir.display()))?;
    }
    diag::ok("TracciaDoc disinstallato (verbali e storico restano in ProgramData)");
    Ok(())
}
