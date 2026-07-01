use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

/// Default chunk size for educational file transfers (64 KB).
pub const CHUNK_SIZE: usize = 64 * 1024;

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Read a file from disk and return base64-encoded chunks with checksums.
pub fn read_file_chunks(path: &str) -> Result<Vec<FileChunkData>, String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("Path is not a regular file".to_string());
    }

    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut chunks = Vec::new();
    let mut index = 0usize;
    let mut buf = vec![0u8; CHUNK_SIZE];

    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        let data = &buf[..n];
        chunks.push(FileChunkData {
            chunk_index: index,
            data_b64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data),
            checksum: sha256_hex(data),
        });
        index += 1;
    }

    Ok(chunks)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileChunkData {
    pub chunk_index: usize,
    pub data_b64: String,
    pub checksum: String,
}

/// Write base64 chunk data to a destination file (append mode after first chunk).
pub fn write_file_chunk(
    dest_path: &str,
    chunk_index: usize,
    data_b64: &str,
    expected_checksum: &str,
) -> Result<(), String> {
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data_b64)
        .map_err(|e| format!("Invalid base64: {}", e))?;

    let actual = sha256_hex(&bytes);
    if actual != expected_checksum {
        return Err(format!(
            "Checksum mismatch for chunk {} (expected {}, got {})",
            chunk_index, expected_checksum, actual
        ));
    }

    if chunk_index == 0 {
        if let Some(parent) = Path::new(dest_path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        let mut file = fs::File::create(dest_path).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
    } else {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(dest_path)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Verify assembled file matches expected full-file checksum.
pub fn verify_file_checksum(path: &str, expected: &str) -> Result<bool, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(sha256_hex(&bytes) == expected)
}
