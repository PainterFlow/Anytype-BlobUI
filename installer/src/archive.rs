use flate2::read::DeflateDecoder;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

const MAX_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;

pub fn extract_zip(bytes: &[u8], destination: &Path) -> Result<(), String> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err("The BlobUI package exceeds the supported size limit.".into());
    }
    if destination.exists() {
        return Err("The installer staging folder already exists.".into());
    }
    fs::create_dir_all(destination)
        .map_err(|e| format!("Could not create installer staging folder: {e}"))?;
    match extract_inner(bytes, destination) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_dir_all(destination);
            Err(error)
        }
    }
}

fn extract_inner(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let eocd = find_eocd(bytes)
        .ok_or_else(|| "The BlobUI package is not a valid ZIP archive.".to_string())?;
    let disk = u16_at(bytes, eocd + 4)?;
    let central_disk = u16_at(bytes, eocd + 6)?;
    let disk_entries = u16_at(bytes, eocd + 8)?;
    let entries = u16_at(bytes, eocd + 10)? as usize;
    let central_size = u32_at(bytes, eocd + 12)? as usize;
    let central_offset = u32_at(bytes, eocd + 16)? as usize;
    let comment = u16_at(bytes, eocd + 20)? as usize;
    if disk != 0
        || central_disk != 0
        || disk_entries as usize != entries
        || entries == u16::MAX as usize
        || central_size == u32::MAX as usize
        || central_offset == u32::MAX as usize
        || eocd + 22 + comment != bytes.len()
        || entries > MAX_ENTRIES
    {
        return Err("The BlobUI package uses an unsupported ZIP layout.".into());
    }
    let central_end = central_offset
        .checked_add(central_size)
        .filter(|end| *end <= eocd)
        .ok_or_else(|| "The BlobUI package has an invalid ZIP directory.".to_string())?;
    let mut cursor = central_offset;
    let mut seen = HashSet::new();
    let mut expanded_total = 0u64;
    for _ in 0..entries {
        if u32_at(bytes, cursor)? != 0x0201_4b50 {
            return Err("The BlobUI package has a malformed ZIP directory.".into());
        }
        let flags = u16_at(bytes, cursor + 8)?;
        let method = u16_at(bytes, cursor + 10)?;
        let crc = u32_at(bytes, cursor + 16)?;
        let compressed = u32_at(bytes, cursor + 20)? as usize;
        let expanded = u32_at(bytes, cursor + 24)? as usize;
        let name_len = u16_at(bytes, cursor + 28)? as usize;
        let extra_len = u16_at(bytes, cursor + 30)? as usize;
        let comment_len = u16_at(bytes, cursor + 32)? as usize;
        let disk_start = u16_at(bytes, cursor + 34)?;
        let external = u32_at(bytes, cursor + 38)?;
        let made_by = u16_at(bytes, cursor + 4)?;
        let local_offset = u32_at(bytes, cursor + 42)? as usize;
        let end = cursor
            .checked_add(46 + name_len + extra_len + comment_len)
            .filter(|end| *end <= central_end)
            .ok_or_else(|| "The BlobUI package has a malformed ZIP directory.".to_string())?;
        if flags & 1 != 0
            || flags & (1 << 6) != 0
            || disk_start != 0
            || compressed == u32::MAX as usize
            || expanded == u32::MAX as usize
        {
            return Err(
                "The BlobUI package contains an encrypted or unsupported ZIP entry.".into(),
            );
        }
        let name = std::str::from_utf8(&bytes[cursor + 46..cursor + 46 + name_len])
            .map_err(|_| "The BlobUI package contains a non-text file path.".to_string())?;
        let (relative, is_dir) = safe_relative(name)?;
        if !seen.insert(relative.to_string_lossy().to_lowercase()) {
            return Err("The BlobUI package contains duplicate file paths.".into());
        }
        let unix_mode = external >> 16;
        if (made_by >> 8 == 3 && unix_mode & 0o170000 == 0o120000) || external & 0x400 != 0 {
            return Err("The BlobUI package contains a link or reparse point.".into());
        }
        if is_dir {
            let dir = destination.join(relative);
            fs::create_dir_all(dir).map_err(|e| format!("Could not create package folder: {e}"))?;
        } else {
            expanded_total = expanded_total
                .checked_add(expanded as u64)
                .filter(|size| *size <= MAX_EXPANDED_BYTES)
                .ok_or_else(|| {
                    "The expanded BlobUI package exceeds the supported size limit.".to_string()
                })?;
            if u32_at(bytes, local_offset)? != 0x0403_4b50 {
                return Err("The BlobUI package has a malformed ZIP file header.".into());
            }
            let local_name = u16_at(bytes, local_offset + 26)? as usize;
            let local_extra = u16_at(bytes, local_offset + 28)? as usize;
            let data_start = local_offset
                .checked_add(30 + local_name + local_extra)
                .filter(|start| {
                    start
                        .checked_add(compressed)
                        .is_some_and(|end| end <= central_offset)
                })
                .ok_or_else(|| "The BlobUI package has a malformed ZIP entry.".to_string())?;
            if u16_at(bytes, local_offset + 6)? != flags
                || u16_at(bytes, local_offset + 8)? != method
                || bytes.get(local_offset + 30..local_offset + 30 + local_name)
                    != Some(name.as_bytes())
            {
                return Err("The BlobUI package has mismatched ZIP entry headers.".into());
            }
            let compressed_bytes = &bytes[data_start..data_start + compressed];
            let decoded = match method {
                0 => compressed_bytes.to_vec(),
                8 => {
                    let mut output = Vec::with_capacity(expanded.min(8 * 1024 * 1024));
                    DeflateDecoder::new(Cursor::new(compressed_bytes))
                        .take(expanded as u64 + 1)
                        .read_to_end(&mut output)
                        .map_err(|_| {
                            "A file in the BlobUI package could not be decompressed.".to_string()
                        })?;
                    output
                }
                _ => {
                    return Err("The BlobUI package uses an unsupported compression method.".into())
                }
            };
            if decoded.len() != expanded || crc32fast::hash(&decoded) != crc {
                return Err(
                    "A file in the BlobUI package failed its size or checksum check.".into(),
                );
            }
            let target = destination.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Could not create package folder: {e}"))?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| format!("Could not write package file: {e}"))?;
            file.write_all(&decoded)
                .map_err(|e| format!("Could not write package file: {e}"))?;
        }
        cursor = end;
    }
    if cursor != central_end {
        return Err("The BlobUI package has trailing ZIP directory data.".into());
    }
    Ok(())
}

fn safe_relative(name: &str) -> Result<(PathBuf, bool), String> {
    let is_dir = name.ends_with('/');
    let value = name.strip_suffix('/').unwrap_or(name);
    if value.is_empty()
        || value.contains('\\')
        || value.contains(':')
        || value.starts_with('/')
        || value.contains('\0')
        || value.chars().any(|c| c.is_control())
    {
        return Err("The BlobUI package contains an unsafe file path.".into());
    }
    for part in value.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|suffix| {
                    matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
                })
            });
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with('.')
            || part.ends_with(' ')
            || part
                .chars()
                .any(|c| matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
            || reserved
        {
            return Err("The BlobUI package contains an unsafe Windows file name.".into());
        }
    }
    let path = Path::new(value);
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err("The BlobUI package contains an unsafe file path.".into());
        }
    }
    Ok((path.to_path_buf(), is_dir))
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 22 {
        return None;
    }
    let start = bytes.len().saturating_sub(65_557);
    (start..=bytes.len() - 22)
        .rev()
        .find(|offset| bytes.get(*offset..*offset + 4) == Some(&[0x50, 0x4b, 0x05, 0x06]))
}
fn u16_at(bytes: &[u8], at: usize) -> Result<u16, String> {
    bytes
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| "The BlobUI package is a truncated ZIP archive.".into())
}
fn u32_at(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| "The BlobUI package is a truncated ZIP archive.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_paths() {
        assert!(safe_relative("../outside").is_err());
        assert!(safe_relative("C:/outside").is_err());
        assert!(safe_relative("folder\\outside").is_err());
    }
}
