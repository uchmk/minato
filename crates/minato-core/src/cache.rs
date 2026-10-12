//! The answers GitHub last gave, kept on disk so a start shows them at once
//! and a thread that has not moved is not asked again. One file per answer:
//! the first line is the key and the stamp (the item's `updatedAt` for a
//! detail, empty otherwise), the rest the body as it came.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// One kept answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub stamp: String,
    pub body: String,
}

/// How long an answer is kept without being written again.
pub const KEEP: Duration = Duration::from_secs(14 * 24 * 3600);

/// The file an answer is kept in: the key with what a file name cannot
/// hold made `_` (the key itself is in the file, so a clash only costs a
/// fetch).
pub fn file_name(key: &str) -> String {
    let safe: String = key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    format!("{safe}.txt")
}

pub fn encode(e: &Entry) -> String {
    format!("{}\t{}\n{}", e.key, e.stamp, e.body)
}

pub fn decode(text: &str) -> Option<Entry> {
    let (head, body) = text.split_once('\n')?;
    let (key, stamp) = head.split_once('\t')?;
    (!key.is_empty()).then(|| Entry { key: key.into(), stamp: stamp.into(), body: body.into() })
}

/// Every answer in `dir` that is young enough; the older ones are removed.
pub fn load(dir: &Path, now: SystemTime) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for f in rd.flatten() {
        let path = f.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let old = f.metadata().and_then(|m| m.modified()).ok().and_then(|t| now.duration_since(t).ok()).is_some_and(|age| age > KEEP);
        if old {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        if let Some(e) = std::fs::read_to_string(&path).ok().as_deref().and_then(decode) {
            out.push(e);
        }
    }
    out
}

/// Where `key` is kept in `dir`.
pub fn path(dir: &Path, key: &str) -> PathBuf {
    dir.join(file_name(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_reads_back() {
        let e = Entry { key: "detail:I_kw/1".into(), stamp: "2026-10-12T00:00:00Z".into(), body: "{\"a\":\n1}".into() };
        assert_eq!(decode(&encode(&e)), Some(e));
        assert_eq!(decode("no newline"), None);
        assert_eq!(decode("\t\nbody"), None);
    }

    #[test]
    fn file_names_are_plain() {
        assert_eq!(file_name("a-runs:uchmk/tsumugi"), "a-runs_uchmk_tsumugi.txt");
        assert_eq!(file_name("detail:PR_kw-1="), "detail_PR_kw-1_.txt");
    }

    #[test]
    fn load_reads_the_young_and_drops_the_old() {
        let dir = std::env::temp_dir().join(format!("minato-cache-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let e = Entry { key: "items".into(), stamp: String::new(), body: "[]".into() };
        std::fs::write(path(&dir, &e.key), encode(&e)).unwrap();
        std::fs::write(dir.join("junk.bin"), "x").unwrap();
        assert_eq!(load(&dir, SystemTime::now()), vec![e]);
        // Far in the future, the file is too old and goes.
        assert!(load(&dir, SystemTime::now() + KEEP + Duration::from_secs(60)).is_empty());
        assert!(!path(&dir, "items").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
