// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  fs::{self, File},
  io::Read,
  path::Path,
};

#[allow(dead_code)]
pub(crate) fn machine_cache_file_is_candidate(path: &Path, max_bytes: u64) -> bool {
  fs::metadata(path)
    .map(|metadata| metadata.is_file() && metadata.len() <= max_bytes)
    .unwrap_or(false)
}

#[allow(dead_code)]
pub(crate) fn read_machine_file_bounded(path: &Path, max_bytes: u64) -> Option<Vec<u8>> {
  let mut file = File::open(path).ok()?;
  let mut bytes = Vec::new();
  file
    .by_ref()
    .take(max_bytes.saturating_add(1))
    .read_to_end(&mut bytes)
    .ok()?;
  if bytes.len() as u64 > max_bytes {
    return None;
  }
  Some(bytes)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bounded_read_accepts_exact_limit() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("cache.bin");
    fs::write(&file, b"abcd").expect("write machine file");

    assert_eq!(read_machine_file_bounded(&file, 4), Some(b"abcd".to_vec()));
  }

  #[test]
  fn bounded_read_rejects_over_limit() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("cache.bin");
    fs::write(&file, b"abcde").expect("write machine file");

    assert_eq!(read_machine_file_bounded(&file, 4), None);
  }

  #[test]
  fn candidate_requires_regular_file_under_limit() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file = dir.path().join("cache.bin");
    fs::write(&file, b"abcd").expect("write machine file");

    assert!(machine_cache_file_is_candidate(&file, 4));
    assert!(!machine_cache_file_is_candidate(&file, 3));
    assert!(!machine_cache_file_is_candidate(dir.path(), 4));
  }
}
