use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use log::{debug, info};

use crate::api::client::RommClient;

pub struct Cache {
    dir: PathBuf,
    ttl: Duration,
    chunk_size: u64,
    lru: Mutex<LruTracker>,
    #[allow(dead_code)]
    max_cached_chunks: usize,
}

struct LruTracker {
    access_order: Vec<String>,
    max_entries: usize,
}

impl LruTracker {
    fn new(max_entries: usize) -> Self {
        Self {
            access_order: Vec::new(),
            max_entries,
        }
    }

    fn touch(&mut self, key: &str) {
        self.access_order.retain(|k| k != key);
        self.access_order.push(key.to_string());
    }

    fn evict_oldest(&mut self, dir: &Path) {
        while self.access_order.len() >= self.max_entries {
            if let Some(oldest) = self.access_order.first().cloned() {
                self.access_order.remove(0);
                let path = dir.join(&oldest);
                if path.exists() {
                    debug!("evicting cached chunk: {oldest}");
                    std::fs::remove_file(&path).ok();
                }
            } else {
                break;
            }
        }
    }
}

fn chunk_key(rom_id: u64, file_name: &str, chunk_idx: u64) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("{rom_id}:{file_name}"));
    let hash = format!("{:x}", hasher.finalize());
    format!("{hash}.{chunk_idx}")
}

impl Cache {
    pub fn new(dir: &Path, ttl: Duration, chunk_size: u64) -> Result<Self> {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating cache dir {}", dir.display()))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            ttl,
            chunk_size,
            lru: Mutex::new(LruTracker::new(10000)),
            max_cached_chunks: 10000,
        })
    }

    pub fn dir_for_statfs(&self) -> &Path {
        &self.dir
    }

    #[allow(dead_code)]
    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    fn chunk_path(&self, rom_id: u64, file_name: &str, chunk_idx: u64) -> PathBuf {
        self.dir.join(chunk_key(rom_id, file_name, chunk_idx))
    }

    fn chunk_valid(&self, path: &Path) -> bool {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return false,
        };
        let modified = match meta.modified() {
            Ok(t) => t,
            Err(_) => return false,
        };
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();
        age <= self.ttl
    }

    fn touch_lru(&self, key: &str) {
        if let Ok(mut lru) = self.lru.lock() {
            lru.touch(key);
        }
    }

    fn evict_if_needed(&self) {
        if let Ok(mut lru) = self.lru.lock() {
            lru.evict_oldest(&self.dir);
        }
    }

    pub fn get_chunk(&self, rom_id: u64, file_name: &str, chunk_idx: u64) -> Option<Vec<u8>> {
        let path = self.chunk_path(rom_id, file_name, chunk_idx);
        if !self.chunk_valid(&path) {
            return None;
        }
        let key = chunk_key(rom_id, file_name, chunk_idx);
        self.touch_lru(&key);
        std::fs::read(&path).ok()
    }

    pub fn download_chunk(
        &self,
        client: &RommClient,
        rom_id: u64,
        file_name: &str,
        chunk_idx: u64,
    ) -> Result<Vec<u8>> {
        let path = self.chunk_path(rom_id, file_name, chunk_idx);
        if self.chunk_valid(&path) {
            let key = chunk_key(rom_id, file_name, chunk_idx);
            self.touch_lru(&key);
            return std::fs::read(&path)
                .with_context(|| format!("reading cached chunk {}", path.display()));
        }

        let start = chunk_idx * self.chunk_size;
        let end = start + self.chunk_size - 1;

        info!(
            "downloading chunk {chunk_idx} for rom {rom_id} file {file_name} (bytes {start}-{end})"
        );
        let data = client
            .download_range(rom_id, file_name, start, end)
            .with_context(|| {
                format!("downloading chunk {chunk_idx} for rom {rom_id} file {file_name}")
            })?;

        let _ = std::fs::write(&path, &data);
        let key = chunk_key(rom_id, file_name, chunk_idx);
        self.touch_lru(&key);
        self.evict_if_needed();

        Ok(data)
    }

    #[allow(dead_code)]
    pub fn chunk_count(&self, file_size: u64) -> u64 {
        file_size.div_ceil(self.chunk_size)
    }

    pub fn read_range(
        &self,
        client: &RommClient,
        rom_id: u64,
        file_name: &str,
        file_size: u64,
        offset: u64,
        size: u32,
    ) -> Result<Vec<u8>> {
        let end_exclusive = offset.saturating_add(size as u64);
        let end_exclusive = std::cmp::min(end_exclusive, file_size);
        if offset >= end_exclusive {
            return Ok(Vec::new());
        }

        let start_chunk = offset / self.chunk_size;
        let end_chunk = (end_exclusive.saturating_sub(1)) / self.chunk_size;

        let mut result = Vec::with_capacity((end_exclusive - offset) as usize);

        for chunk_idx in start_chunk..=end_chunk {
            let chunk_start = chunk_idx * self.chunk_size;
            let chunk_end_inclusive = std::cmp::min(
                chunk_start + self.chunk_size - 1,
                file_size.saturating_sub(1),
            );

            let data = match self.get_chunk(rom_id, file_name, chunk_idx) {
                Some(cached) => cached,
                None => self.download_chunk(client, rom_id, file_name, chunk_idx)?,
            };

            let copy_start_in_chunk = if chunk_start < offset {
                (offset - chunk_start) as usize
            } else {
                0
            };
            let copy_end_in_chunk = if end_exclusive <= chunk_end_inclusive {
                (end_exclusive - chunk_start) as usize
            } else {
                data.len()
            };

            if copy_start_in_chunk < data.len() && copy_start_in_chunk < copy_end_in_chunk {
                let slice_end = std::cmp::min(copy_end_in_chunk, data.len());
                result.extend_from_slice(&data[copy_start_in_chunk..slice_end]);
            }
        }

        Ok(result)
    }

    #[allow(dead_code)]
    pub fn delete_all_chunks(&self, rom_id: u64, file_name: &str) {
        let prefix = chunk_key(rom_id, file_name, 0);
        let prefix = prefix.trim_end_matches('0');
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(prefix)
                        || name.starts_with(&format!("{rom_id}:{file_name}"))
                    {
                        std::fs::remove_file(entry.path()).ok();
                    }
                }
            }
        }
    }
}
