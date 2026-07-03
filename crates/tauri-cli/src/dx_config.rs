use std::collections::HashMap;
use std::path::PathBuf;

pub struct NativeDxConfig {
    pub workspace_root: PathBuf,
    pub sr_dir: PathBuf,
    pub receipts_dir: PathBuf,
}

impl NativeDxConfig {
    pub fn load() -> Self {
        let ws = discover_workspace_root().unwrap_or_else(|| PathBuf::from("."));
        let sr = ws.join(".dx").join("serializer");
        let receipts = ws.join(".dx").join("receipts");
        Self { workspace_root: ws, sr_dir: sr, receipts_dir: receipts }
    }

    pub fn ensure_dirs(&self) {
        let _ = std::fs::create_dir_all(&self.sr_dir);
        let _ = std::fs::create_dir_all(&self.receipts_dir);
    }

    pub fn sr_path(&self, name: &str) -> PathBuf {
        self.sr_dir.join(format!("{name}.sr"))
    }

    pub fn write_sr(&self, name: &str, entries: &[(&str, &str)]) -> std::io::Result<()> {
        let path = self.sr_path(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf: Vec<u8> = Vec::new();
        for (key, value) in entries {
            write!(buf, "{key}=")?;
            write_llm_value(&mut buf, value)?;
            buf.push(b'\n');
        }
        let tmp = path.with_extension("sr.tmp");
        std::fs::write(&tmp, &buf)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn global_sr_dir(&self) -> PathBuf {
        dirs::cache_dir().map(|b| b.join("dx").join("native")).unwrap_or_else(|| PathBuf::from("~/.cache/dx/native"))
    }

    pub fn write_global_sr(&self, name: &str, entries: &[(&str, &str)]) -> std::io::Result<()> {
        let path = self.global_sr_dir().join(format!("{name}.sr"));
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut buf: Vec<u8> = Vec::new();
        for (key, value) in entries {
            write!(buf, "{key}=")?;
            write_llm_value(&mut buf, value)?;
            buf.push(b'\n');
        }
        let tmp = path.with_extension("sr.tmp");
        std::fs::write(&tmp, &buf)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn read_status(&self, name: &str) -> Option<HashMap<String, String>> {
        let path = self.sr_path(name);
        let source = std::fs::read_to_string(&path).ok()?;
        let mut map = HashMap::new();
        for line in source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        Some(map)
    }
}

fn write_llm_value(buf: &mut Vec<u8>, value: &str) -> std::io::Result<()> {
    if value.is_empty() {
        buf.push(b'"');
        buf.push(b'"');
        return Ok(());
    }
    let needs_quoting = value.contains('"')
        || value.contains('\\')
        || value.contains('\n')
        || value.contains('\r')
        || value.starts_with('"')
        || value.contains(' ')
        || value.contains('\t')
        || value.contains('#');
    if needs_quoting {
        buf.push(b'"');
        for ch in value.chars() {
            match ch {
                '"' | '\\' => {
                    buf.push(b'\\');
                    buf.push(ch as u8);
                }
                '\n' => buf.extend_from_slice(b"\\n"),
                '\r' => buf.extend_from_slice(b"\\r"),
                '\t' => buf.extend_from_slice(b"\\t"),
                _ => buf.extend(ch.encode_utf8(&mut [0; 4]).as_bytes()),
            }
        }
        buf.push(b'"');
    } else {
        buf.extend_from_slice(value.as_bytes());
    }
    Ok(())
}

fn discover_workspace_root() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    for ancestor in cwd.ancestors() {
        let candidate = ancestor.join("dx");
        if candidate.is_file() {
            let source = std::fs::read_to_string(&candidate).ok()?;
            let first = source.lines().find(|l| {
                let t = l.trim().trim_start_matches('\u{feff}');
                !t.is_empty() && !t.starts_with('#')
            })?;
            if !first.starts_with("project(") && !first.starts_with("contract(") &&
               !first.starts_with("runtime(") && !first.starts_with("www(") {
                if let Some(line) = source.lines().find(|l| l.trim().starts_with("workspace.root")) {
                    if let Some(val) = line.split('=').nth(1) {
                        let p = val.trim().trim_matches('"').trim();
                        let path = PathBuf::from(p);
                        if path.is_absolute() { return Some(path); }
                        return Some(ancestor.join(path));
                    }
                }
                return Some(ancestor.to_path_buf());
            }
        }
    }
    None
}
