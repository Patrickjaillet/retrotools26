use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Downloads `url` into `dest` unless a file already exists there (repeat
/// scrape runs — including one resumed after being interrupted — never
/// re-download media that's already cached). Returns `true` if a download
/// actually happened.
pub fn download_if_missing(url: &str, dest: &Path) -> Result<bool, String> {
    if dest.is_file() {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let response = ureq::get(url).timeout(Duration::from_secs(30)).call().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    std::fs::write(dest, &bytes).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Total size in bytes of every file under `dir`.
pub fn cache_size(dir: &Path) -> u64 {
    walk_files(dir).iter().filter_map(|p| p.metadata().ok()).map(|m| m.len()).sum()
}

/// Deletes the oldest (by modification time) files under `dir` until its
/// total size is at or below `max_bytes`. A simple age-based purge rather
/// than a true LRU (no access-time tracking beyond what the filesystem
/// already gives us for free), which is enough for "don't let the cache
/// grow forever" without adding a tracking database.
pub fn purge_to_limit(dir: &Path, max_bytes: u64) -> std::io::Result<usize> {
    let mut files: Vec<(PathBuf, std::time::SystemTime, u64)> = walk_files(dir)
        .into_iter()
        .filter_map(|p| {
            let meta = p.metadata().ok()?;
            let modified = meta.modified().ok()?;
            Some((p, modified, meta.len()))
        })
        .collect();
    files.sort_by_key(|(_, modified, _)| *modified);

    let mut total: u64 = files.iter().map(|(_, _, size)| size).sum();
    let mut removed = 0;
    for (path, _, size) in &files {
        if total <= max_bytes {
            break;
        }
        std::fs::remove_file(path)?;
        total = total.saturating_sub(*size);
        removed += 1;
    }
    Ok(removed)
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::net::TcpListener;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-scraper-cache-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn serve_once(body: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 512];
            let _ = stream.read(&mut buf);
            let header = format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(body);
        });
        format!("http://127.0.0.1:{port}/image.png")
    }

    #[test]
    fn downloads_once_then_skips_on_repeat_calls() {
        let dir = temp_dir("download");
        let dest = dir.join("box.png");
        let url = serve_once(b"fake-image-bytes");

        assert!(download_if_missing(&url, &dest).unwrap());
        assert_eq!(std::fs::read(&dest).unwrap(), b"fake-image-bytes");
        assert!(!download_if_missing(&url, &dest).unwrap(), "second call must not re-download");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn purge_removes_oldest_files_first_until_under_the_limit() {
        let dir = temp_dir("purge");
        let old = dir.join("old.png");
        let newer = dir.join("newer.png");
        std::fs::write(&old, vec![0u8; 100]).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&newer, vec![0u8; 100]).unwrap();

        assert_eq!(cache_size(&dir), 200);
        let removed = purge_to_limit(&dir, 150).unwrap();
        assert_eq!(removed, 1);
        assert!(!old.exists(), "the older file should be removed first");
        assert!(newer.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn purge_does_nothing_when_already_under_the_limit() {
        let dir = temp_dir("purge-noop");
        std::fs::write(dir.join("small.png"), vec![0u8; 10]).unwrap();
        let removed = purge_to_limit(&dir, 1_000_000).unwrap();
        assert_eq!(removed, 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
