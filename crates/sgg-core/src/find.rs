use std::path::{Path, PathBuf};

/// Walk upward from `start` until a `.git` entry is found.
pub fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let git_entry = current.join(".git");
        if git_entry.exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_nearest_git_root() {
        let tmp = std::env::temp_dir().join(format!("sgg-test-{}", std::process::id()));
        let nested = tmp.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(tmp.join(".git")).unwrap();
        assert_eq!(find_git_root(&nested), Some(tmp.clone()));
        let _ = fs::remove_dir_all(tmp);
    }
}