use std::{
    fs,
    io::{self, Write},
};

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

pub enum HashObjectMode<'a> {
    HashOnly,
    Write(&'a Repo),
}

pub fn hash_object(
    mode: HashObjectMode,
    object: &mut dyn io::Read,
    stdout: &mut dyn io::Write,
) -> Result<()> {
    let mut data = Vec::new();
    object.read_to_end(&mut data)?;
    let blob = object::Blob::new(data);
    let hash = blob.hash();

    if let HashObjectMode::Write(repo) = mode {
        let dir = &repo.git_dir().join("objects").join(&hash[0..2]);
        let file_path = dir.join(&hash[2..]);
        let mut data = Vec::new();
        let mut writer = flate2::write::ZlibEncoder::new(&mut data, flate2::Compression::default());
        writer.write_all(b"blob ")?;
        writer.write_all(blob.content.len().to_string().as_bytes())?;
        writer.write_all(b"\0")?;
        writer.write_all(&blob.content)?;
        drop(writer);
        fs::create_dir_all(dir)?;
        fs::write(file_path, data)?;
    }

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

pub fn log(repo: &Repo, object_rev: &str, stdout: &mut dyn io::Write) -> Result<()> {
    let mut next_object_rev = Some(object_rev.to_string());

    while let Some(this_rev) = &next_object_rev {
        let current_object = Object::from_rev(repo, this_rev)?;

        match current_object {
            Object::Blob(_) => {
                return Ok(());
            }
            Object::Tree(_) => {
                return Ok(());
            }
            Object::Commit(commit) => {
                let commiter = commit.committer;
                let first_line = commit.message.lines().next().unwrap_or("");
                writeln!(
                    stdout,
                    "{hash} - {first_line} - \"{commiter}\"",
                    hash = &this_rev[0..6]
                )?;
                if commit.parent.is_empty() {
                    return Ok(());
                } else {
                    next_object_rev = Some(commit.parent.clone());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
        hash_object(
            HashObjectMode::HashOnly,
            &mut "test content\n".as_bytes(),
            &mut stdout,
        )
        .unwrap();
        assert_eq!(stdout, b"d670460b4b4aece5915caf5c68d12f560a9fe3e4\n");
    }
}
