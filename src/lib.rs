use std::{fs, io};

use anyhow::Result;
use object::Object;
use repo::Repo;

pub mod object;
pub mod repo;

pub fn init_repo(repo: &Repo, branch_name: &str) -> Result<()> {
    let repo_path = &repo.root;
    let git_folder = repo.git_dir();
    println!("Initializing repo {repo_path:?} with branch {branch_name}");

    fs::create_dir_all(repo_path)?;
    fs::create_dir_all(&git_folder)?;
    fs::create_dir_all(git_folder.join("objects"))?;
    fs::create_dir_all(git_folder.join("refs/heads"))?;

    let data = format!("ref: refs/heads/{branch_name}");
    fs::write(git_folder.join("HEAD"), data)?;
    Ok(())
}

pub fn hash_object(object: &mut dyn io::Read, stdout: &mut dyn io::Write) -> Result<()> {
    let mut data = Vec::new();
    object.read_to_end(&mut data)?;
    let blob = object::Blob::new(data);
    let hash = blob.hash();

    writeln!(stdout, "{hash}")?;
    Ok(())
}

pub fn cat_file(repo: &Repo, object_hash: &str, stdout: &mut dyn io::Write) -> Result<()> {
    let object = Object::from_rev(repo, object_hash)?;

    match object {
        Object::Blob(blob) => {
            let content = std::str::from_utf8(&blob.content)?;
            writeln!(stdout, "{content}")?;
        }
        Object::Tree(tree) => {
            for file in tree.files {
                writeln!(
                    stdout,
                    "{:>6} {:>4} {:43} {}",
                    file.mode,
                    file.type_str(),
                    file.hash,
                    file.name
                )?;
            }
        }
        Object::Commit(commit) => {
            writeln!(stdout, "tree: {}", commit.tree)?;
            writeln!(stdout, "parent: {}", commit.parent)?;
            writeln!(stdout, "author: {}", commit.author)?;
            writeln!(stdout, "committer: {}", commit.committer)?;
            writeln!(stdout, "\n{}", commit.message)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::prelude::*;

    use super::*;

    #[test]
    fn test_init_repo() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().to_path_buf();
        init_repo(&Repo::new(&path), "bestbranch").unwrap();
        assert_eq!(
            fs::read_to_string(path.join(".git/HEAD")).unwrap(),
            "ref: refs/heads/bestbranch"
        );
    }

    #[test]
    fn test_hash_object() {
        let mut stdout = Vec::new();

        // From https://git-scm.com/book/sv/v2/Git-Internals-Git-Objects
        hash_object(&mut "test content\n".as_bytes(), &mut stdout).unwrap();
        assert_eq!(stdout, b"d670460b4b4aece5915caf5c68d12f560a9fe3e4\n");
    }

    #[test]
    fn test_cat_file() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir
            .path()
            .to_path_buf()
            .join(".git/objects/d6/70460b4b4aece5915caf5c68d12f560a9fe3e4");
        let mut stdout = Vec::new();

        let prefix = path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();

        // Compress the content of the blob object and write to file
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(b"blob 13\0test content\n").unwrap();
        let compressed = e.finish().unwrap();
        std::fs::write(&path, compressed).unwrap();

        cat_file(
            &Repo::new(tmpdir.path()),
            "d670460b4b4aece5915caf5c68d12f560a9fe3e4",
            &mut stdout,
        )
        .unwrap();
        assert_eq!(stdout, b"test content\n\n");
    }

    #[test]
    fn test_cat_file_short_hash() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir
            .path()
            .to_path_buf()
            .join(".git/objects/d6/70460b4b4aece5915caf5c68d12f560a9fe3e4");
        let mut stdout = Vec::new();

        let prefix = path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();

        // Compress the content of the blob object and write to file
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(b"blob 13\0test content\n").unwrap();
        let compressed = e.finish().unwrap();
        std::fs::write(&path, compressed).unwrap();

        cat_file(&Repo::new(tmpdir.path()), "d670", &mut stdout).unwrap();
        assert_eq!(stdout, b"test content\n\n");
    }

    #[test]
    fn test_cat_file_fails_if_rev_not_found() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir
            .path()
            .to_path_buf()
            .join(".git/objects/d6/70460b4b4aece5915caf5c68d12f560a9fe3e4");
        let mut stdout = Vec::new();

        let prefix = path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();

        let err = cat_file(&Repo::new(tmpdir.path()), "d6", &mut stdout)
            .unwrap_err()
            .to_string();
        assert_eq!(err, "Object not found");
    }
}
