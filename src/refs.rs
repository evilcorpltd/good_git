use std::fs;

use crate::repo::Repo;

use anyhow::{Result, anyhow};

/// Finds and resolves a Git reference to its commit hash.
///
/// # Arguments
/// * reference - The reference path to look up, prefixed with the type of reference (e.g. refs/heads)
/// * repo - The repository to search in
///
/// # Returns
/// * The commit hash as a string if found
pub fn find_ref(reference: &str, repo: &Repo) -> Result<String> {
    let path = repo.git_dir().join(reference);
    if !path.exists() {
        return Err(anyhow!("Reference not found: {reference}"));
    }
    let content = fs::read_to_string(path)?;
    if content.starts_with("ref: ") {
        let target = content.trim_start_matches("ref: ").trim_end();
        return find_ref(target, repo);
    }
    Ok(content.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    struct TestRepo {
        dir: tempfile::TempDir,
        repo: Repo,
    }

    impl TestRepo {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            fs::create_dir_all(dir.path().join(".git").join("refs").join("heads")).unwrap();
            let repo = Repo::new(dir.path());
            TestRepo { dir, repo }
        }

        fn create_ref(&self, name: &str, content: &str) {
            let ref_path = self.dir.path().join(".git/refs/heads").join(name);
            fs::write(ref_path, content).unwrap();
        }
    }

    #[test]
    fn test_find_ref_existing() {
        let test_repo = TestRepo::new();
        test_repo.create_ref("main", "commit_hash\n");

        let result = find_ref("refs/heads/main", &test_repo.repo);
        assert_eq!(result.unwrap(), "commit_hash");
    }

    #[test]
    fn test_find_ref_non_existing() {
        let test_repo = TestRepo::new();

        let result = find_ref("non_existing", &test_repo.repo);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert_eq!(err, "Reference not found: non_existing");
    }

    #[test]
    fn test_find_ref_with_reference() {
        let test_repo = TestRepo::new();
        test_repo.create_ref("main", "ref: refs/heads/feature\n");
        test_repo.create_ref("feature", "commit_hash");

        let result = find_ref("refs/heads/main", &test_repo.repo);
        assert_eq!(result.unwrap(), "commit_hash");
    }
}
