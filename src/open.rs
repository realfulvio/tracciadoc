use crate::{engine, model::Finding};
use std::{os::windows::process::CommandExt, path::Path, process::Command};

fn safe_extension(f: &Finding) -> bool {
    let ext = f
        .path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match f.format.to_ascii_lowercase().as_str() {
        "pdf" => ext == "pdf",
        "png" => ext == "png",
        "jpeg" => ["jpg", "jpeg"].contains(&ext.as_str()),
        "bmp" => ext == "bmp",
        "webp" => ext == "webp",
        "gif" => ext == "gif",
        "tiff" => ["tif", "tiff"].contains(&ext.as_str()),
        "office xml / zip" => ["docx", "xlsx", "pptx", "odt", "ods", "odp"].contains(&ext.as_str()),
        "testo utf-8" => ["txt", "csv", "tsv", "log", "md", "rtf"].contains(&ext.as_str()),
        _ => false,
    }
}
pub fn document(f: &Finding) -> Result<(), String> {
    if engine::is_network(&f.path) || !f.path.is_file() {
        return Err("File non piu disponibile localmente".into());
    }
    if !safe_extension(f) {
        return Err("Estensione assente o diversa dal formato rilevato: usa Mostra nella cartella e scegli manualmente un visualizzatore. Nessun programma viene eseguito in base a un'estensione ambigua.".into());
    }
    let path: Vec<u16> = f
        .path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize <= 32 {
        Err(
            "Nessun visualizzatore associato o apertura non riuscita; usa Mostra nella cartella"
                .into(),
        )
    } else {
        Ok(())
    }
}
pub fn folder(path: &Path) -> Result<(), String> {
    if engine::is_network(path) {
        return Err("Percorso di rete escluso".into());
    }
    let explorer =
        std::path::PathBuf::from(std::env::var_os("WINDIR").ok_or("Windows non individuato")?)
            .join("explorer.exe");
    Command::new(explorer)
        .arg("/select,")
        .arg(path)
        .creation_flags(0x08000000)
        .spawn()
        .map(|_| ())
        .map_err(|_| "Impossibile aprire Esplora file".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn do_not_execute_disguised_files() {
        let f = Finding {
            path: "documento.exe".into(),
            format: "PDF".into(),
            reasons: vec![],
            ..Default::default()
        };
        assert!(!safe_extension(&f));
        assert!(safe_extension(&Finding {
            path: "documento.pdf".into(),
            ..f
        }));
    }
}
