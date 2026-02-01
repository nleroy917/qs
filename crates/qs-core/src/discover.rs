//! Discovery module: Find .qs root by walking up the directory tree

use std::path::{Path, PathBuf};

use crate::{QS_DIR, QsError, Result};

/// Find the .qs root directory by walking up from the given path.
///
/// Returns the path to the directory containing .qs (not the .qs folder itself).
pub fn find_qs_root(start: &Path) -> Result<PathBuf> {
    let mut current = start.canonicalize()?;

    loop {
        let qs_path = current.join(QS_DIR);
        if qs_path.is_dir() {
            return Ok(current);
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return Err(QsError::NotInRepo),
        }
    }
}

/// Get the .qs directory path for a given root.
pub fn qs_dir(root: &Path) -> PathBuf {
    root.join(QS_DIR)
}

/// Get the shard directory path.
pub fn shard_dir(root: &Path) -> PathBuf {
    qs_dir(root).join("shard")
}

/// Get the config file path.
pub fn config_path(root: &Path) -> PathBuf {
    qs_dir(root).join("config.json")
}

/// Get the files metadata path.
pub fn files_path(root: &Path) -> PathBuf {
    qs_dir(root).join("files.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_qs_root() {
        let temp = std::env::temp_dir().join("qs_test_discover");
        let _ = fs::remove_dir_all(&temp);

        let nested = temp.join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(temp.join(QS_DIR)).unwrap();

        let found = find_qs_root(&nested).unwrap();
        assert_eq!(found, temp.canonicalize().unwrap());

        fs::remove_dir_all(&temp).unwrap();
    }

    #[test]
    fn test_not_in_repo() {
        let temp = std::env::temp_dir().join("qs_test_no_repo");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        let result = find_qs_root(&temp);
        assert!(matches!(result, Err(QsError::NotInRepo)));

        fs::remove_dir_all(&temp).unwrap();
    }
}
