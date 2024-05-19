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
        Some(Repo::new(path))
    }

    pub fn git_dir(&self) -> std::path::PathBuf {
        self.root.join(GIT_FOLDER_NAME)
    }
}
