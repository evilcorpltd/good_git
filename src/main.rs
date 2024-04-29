use anyhow::Result;
use std::{fs, path::PathBuf};

use clap::{Args, Parser, Subcommand};

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
}

#[derive(Args)]
struct InitArgs {
    #[arg(default_value = ".")]
    path: PathBuf,

    #[arg(default_value = "master")]
    branch: String,
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(init_args) => {
            init_repo(&init_args.path, &init_args.branch)?;
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
}
