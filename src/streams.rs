//! Detect named NTFS data streams without reading, recalling or deleting their contents.
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows_sys::Win32::{
    Foundation::{GetLastError, ERROR_HANDLE_EOF, ERROR_INVALID_PARAMETER, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{FindClose, FindFirstStreamW, FindNextStreamW, FindStreamInfoStandard, WIN32_FIND_STREAM_DATA},
};

pub const NOT_ANALYZED: &str = "Flussi alternativi NTFS presenti: contenuto dei flussi non analizzato";
pub const ENUMERATION_FAILED: &str = "Flussi alternativi NTFS: enumerazione non riuscita";

pub fn has_named_data(path: &Path) -> Result<bool, &'static str> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: a terminated path and a writable native output structure, valid for the calls.
    unsafe {
        let mut data: WIN32_FIND_STREAM_DATA = std::mem::zeroed();
        let handle = FindFirstStreamW(wide.as_ptr(), FindStreamInfoStandard, &mut data as *mut _ as _, 0);
        if handle == INVALID_HANDLE_VALUE {
            return match GetLastError() {
                // Directories without streams, or a filesystem without stream support.
                ERROR_HANDLE_EOF | ERROR_INVALID_PARAMETER => Ok(false),
                _ => Err(ENUMERATION_FAILED),
            };
        }
        let result = loop {
            let n = data.cStreamName.iter().position(|&c| c == 0).unwrap_or(data.cStreamName.len());
            if data.cStreamName[..n] != "::$DATA".encode_utf16().collect::<Vec<_>>() {
                break Ok(true);
            }
            if FindNextStreamW(handle, &mut data as *mut _ as _) == 0 {
                break if GetLastError() == ERROR_HANDLE_EOF { Ok(false) } else { Err(ENUMERATION_FAILED) };
            }
        };
        FindClose(handle);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_streams_on_files_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ordinary.txt");
        std::fs::write(&file, b"synthetic host").unwrap();
        assert!(!has_named_data(&file).unwrap());
        std::fs::write(format!("{}:synthetic.png", file.display()), b"synthetic stream").unwrap();
        assert!(has_named_data(&file).unwrap());
        assert!(!has_named_data(dir.path()).unwrap());
        std::fs::write(format!("{}:synthetic.pdf", dir.path().display()), b"synthetic stream").unwrap();
        assert!(has_named_data(dir.path()).unwrap());
    }
}
