use anyhow::Result;
use std::{fs, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use std::io;

mod object;

#[derive(Parser)]
#[command(version)]
#[command(about = "A good git client", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new empty repo.
    Init(InitArgs),

    /// Calculates the hash of an object.
    HashObject(HashObjectArgs),
}

#[derive(Args)]
struct InitArgs {
    #[arg(default_value = ".")]
    path: PathBuf,

    #[arg(default_value = "master")]
    branch: String,
}

#[derive(Args)]
struct HashObjectArgs {
    file: PathBuf,
}

fn init_repo(path: &PathBuf, branch_name: &str) -> Result<()> {
    let repo_path = path;
    let git_folder = path.join(".git");
    println!("Initializing repo {repo_path:?} with branch {branch_name}");

    fs::create_dir_all(repo_path)?;
    fs::create_dir_all(&git_folder)?;
    fs::create_dir_all(git_folder.join("objects"))?;
    fs::create_dir_all(git_folder.join("refs/heads"))?;

    let data = format!("ref: refs/heads/{branch_name}");
    fs::write(git_folder.join("HEAD"), data)?;
    Ok(())
}

fn hash_object(file: &PathBuf, stdout: &mut dyn io::Write) -> Result<()> {
    let data = std::fs::read(file)?;
    let blob = object::Blob::new(data);
    let hash = blob.hash();

    writeln!(stdout, "{hash}")?;
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(init_args) => {
            init_repo(&init_args.path, &init_args.branch)?;
        }
        Commands::HashObject(hash_object_args) => {
            hash_object(&hash_object_args.file, &mut io::stdout())?;
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
        init_repo(&path, "bestbranch").unwrap();
        assert_eq!(
            fs::read_to_string(path.join(".git/HEAD")).unwrap(),
            "ref: refs/heads/bestbranch"
        );
    }

    #[test]
    fn test_hash_object() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().to_path_buf().join("file.txt");
        let mut stdout = Vec::new();

        // From https://git-scm.com/book/sv/v2/Git-Internals-Git-Objects
        std::fs::write(&path, b"test content\n").ok();
        hash_object(&path, &mut stdout).unwrap();
        assert_eq!(stdout, b"d670460b4b4aece5915caf5c68d12f560a9fe3e4\n");
    }
}
