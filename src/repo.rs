use std::fs;

static GIT_FOLDER_NAME: &str = ".git";

#[derive(Debug)]
pub struct Repo {
    pub root: std::path::PathBuf,
}

impl Repo {
    pub fn new(root: &std::path::Path) -> Self {
        Repo {
            root: root.to_path_buf(),
        }
    }

    pub fn from_dir(path: &std::path::Path) -> Option<Self> {
        let path = fs::canonicalize(path).ok()?;
        let git_dir = path.ancestors().find(|&d| {
            let git_dir = d.join(GIT_FOLDER_NAME);
            git_dir.exists()
        });
        git_dir.map(Repo::new)
    }

    pub fn git_dir(&self) -> std::path::PathBuf {
        self.root.join(GIT_FOLDER_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_dir_in_sub_dir() {
        let tmpdir = tempfile::tempdir()
            .unwrap()
            .path()
            .to_path_buf()
            .canonicalize()
            .unwrap();
        let git_dir = tmpdir.join(GIT_FOLDER_NAME);
        std::fs::create_dir_all(&git_dir).unwrap();

        let cwd_dir = tmpdir.join("hello/from/nested/dir");
        std::fs::create_dir_all(&cwd_dir).unwrap();

        let repo = Repo::from_dir(&cwd_dir).unwrap();

        assert_eq!(tmpdir, repo.root);
        assert_eq!(git_dir, repo.git_dir());
    }
}
