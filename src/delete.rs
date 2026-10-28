use crate::{engine, model::Finding};
use std::fs;

/// Eliminazione definitiva (non passa dal cestino) di un file segnalato e selezionato dall'operatore.
/// Rifiuta reti, collegamenti simbolici/junction e file diversi da quelli regolari.
#[allow(clippy::permissions_set_readonly_false)] // app solo Windows
pub fn remove(f: &Finding) -> Result<(), String> {
    if engine::is_network(&f.path) {
        return Err("Percorso di rete escluso".into());
    }
    let meta = fs::symlink_metadata(&f.path).map_err(|e| format!("File non accessibile: {e}"))?;
    if !meta.file_type().is_file() {
        return Err("Non e un file regolare (collegamento o cartella): non eliminato".into());
    }
    let mut perms = meta.permissions();
    if perms.readonly() {
        perms.set_readonly(false);
        fs::set_permissions(&f.path, perms).map_err(|e| format!("Attributo sola lettura: {e}"))?;
    }
    fs::remove_file(&f.path).map_err(|e| format!("Eliminazione non riuscita: {e}"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_regular_file_only() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("prova.txt");
        fs::write(&file, b"sintetico").unwrap();
        let mut f = Finding {
            path: file.clone(),
            ..Default::default()
        };
        assert!(remove(&f).is_ok());
        assert!(!file.exists());
        assert!(remove(&f).is_err());
        f.path = dir.path().to_path_buf();
        assert!(remove(&f).is_err());
        assert!(dir.path().exists());
    }
}
