use anyhow::{Result, anyhow};
use good_git::{hash_object, repo::Repo};
use std::{fs, path::Path, path::PathBuf};

use clap::{Args, Parser, Subcommand};
use std::io;

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

    /// Prints contents of an object.
    CatFile(CatFileArgs),

    /// Show a log of the history.
    Log(LogArgs),
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
    /// Write the object into the object database.
    #[arg(short)]
    write: bool,

    /// Read the object from stdin instead of from a file.
    #[arg(long)]
    stdin: bool,

    #[arg(required_unless_present("stdin"))]
    file: Option<PathBuf>,
}

#[derive(Args)]
struct CatFileArgs {
    object: String,
}

#[derive(Args)]
struct LogArgs {
    object: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init(init_args) => {
            good_git::init_repo(&Repo::new(&init_args.path), &init_args.branch)?;
        }
        Commands::HashObject(hash_object_args) => {
            let repo = Repo::from_dir(Path::new("."));
            let mode = if hash_object_args.write {
                good_git::HashObjectMode::Write(
                    repo.as_ref()
                        .ok_or_else(|| anyhow!("Could not find a valid git repository"))?,
                )
            } else {
                good_git::HashObjectMode::HashOnly
            };

            if hash_object_args.stdin {
                hash_object(mode, &mut io::stdin(), &mut io::stdout())?;
            } else {
                let f = hash_object_args
                    .file
                    .clone()
                    .expect("<file> is required when --stdin isn't set");
                let f = fs::File::open(f)?;
                hash_object(mode, &mut io::BufReader::new(f), &mut io::stdout())?;
            }
        }
        Commands::CatFile(cat_file_args) => {
            let repo = Repo::from_dir(Path::new("."))
                .ok_or_else(|| anyhow!("Could not find a valid git repository"))?;
            good_git::cat_file(&repo, &cat_file_args.object, &mut io::stdout())?;
        }
        Commands::Log(log_args) => {
            let repo = Repo::from_dir(Path::new("."))
                .ok_or_else(|| anyhow!("Could not find a valid git repository"))?;
            good_git::log(&repo, &log_args.object, &mut io::stdout())?;
        }
    }
    Ok(())
}
