use flate2::{write::ZlibEncoder, Compression};
use good_git::object::{Commit, Tree};
use good_git::repo::Repo;
use rstest::fixture;
use std::io::prelude::*;
use std::path::PathBuf;

fn write_compressed_object(dir: PathBuf, hash: &str, object_content: &[u8]) {
    let (short_hash, long_hash) = hash.split_at(2);
    let path = dir.join(".git/objects").join(short_hash).join(long_hash);

    let prefix = path.parent().unwrap();
    std::fs::create_dir_all(prefix).unwrap();

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(object_content).unwrap();
    let compressed = encoder.finish().unwrap();
    std::fs::write(path, compressed).unwrap();
}

fn create_blob(dir: PathBuf, hash: &str, content: &str) {
    write_compressed_object(
        dir,
        hash,
        format!("blob {}\0{}", content.len(), content).as_bytes(),
    );
}

fn create_tree(dir: PathBuf, hash: &str, tree: &Tree) {
    // Format (one per file/folder/tree/submodule):
    // [mode] [object name]\0[SHA-1 in binary format (20 bytes)]
    let tree_content: Vec<u8> = tree
        .files
        .iter()
        .flat_map(|file| {
            let mut bytes = file.mode.as_bytes().to_vec();
            bytes.push(b' ');
            bytes.extend(file.name.as_bytes());
            bytes.push(0);
            bytes.extend(&hex::decode(&file.hash).unwrap());
            bytes
        })
        .collect();

    let tree_header = format!("tree {}\0", tree_content.len()).into_bytes();
    let full_bytes = [tree_header, tree_content].concat();

    write_compressed_object(dir, hash, &full_bytes);
}

fn create_commit(dir: PathBuf, hash: &str, commit: &Commit) {
    // Format is:
    // [key] [value]
    // ...
    // <empty line>
    // [commit message]
    let content = format!(
        "\
tree {}
encoding {}
committer {}
author {}
parent {}

{}",
        commit.tree,
        commit.encoding,
        commit.committer,
        commit.author,
        commit.parent,
        commit.message
    )
    .into_bytes();

    let header = format!("commit {}\0", content.len()).into_bytes();
    let full_bytes = [header, content].concat();

    write_compressed_object(dir, hash, &full_bytes);
}

#[fixture]
fn test_repo() -> tempfile::TempDir {
    let tmpdir = tempfile::tempdir().unwrap();
    let git_dir = tmpdir.path().to_path_buf();

    good_git::init_repo(&Repo::new(&git_dir), "main").unwrap();

    create_blob(
        git_dir.clone(),
        "d670460b4b4aece5915caf5c68d12f560a9fe3e4",
        "test content\n",
    );

    create_blob(
        git_dir.clone(),
        "1234567890abcdef1234567890abcdef12345678",
        "more content\nfrom a good client",
    );

    let tree = Tree {
        files: vec![
            good_git::object::File {
                mode: "100644".to_string(),
                hash: "d670460b4b4aece5915caf5c68d12f560a9fe3e4".to_string(),
                name: "test.txt".to_string(),
            },
            good_git::object::File {
                mode: "100644".to_string(),
                hash: "1234567890abcdef1234567890abcdef12345678".to_string(),
                name: "more.txt".to_string(),
            },
        ],
    };
    create_tree(
        git_dir.clone(),
        "99887766554433221100aabbccddeeff00112233",
        &tree,
    );

    let commit = Commit {
        tree: "99887766554433221100aabbccddeeff00112233".to_string(),
        parent: "".to_string(),
        author: "Bob <hello@bob.test>".to_string(),
        committer: "Alice <bye@alice.test>".to_string(),
        encoding: "".to_string(),
        message: "This is a good commit".to_string(),
    };
    create_commit(
        git_dir.clone(),
        "aaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbbbbb",
        &commit,
    );

    let commit = Commit {
        tree: "99887766554433221100aabbccddeeff00112233".to_string(),
        parent: "aaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbbbbb".to_string(),
        author: "Captain Nemo <nemo@nautilus.sea>".to_string(),
        committer: "Sherlock Holmes <sherlock@baker.street>".to_string(),
        encoding: "".to_string(),
        message: "Here is a better commit".to_string(),
    };
    create_commit(
        git_dir.clone(),
        "ccccccccccccccccccccdddddddddddddddddddd",
        &commit,
    );

    tmpdir
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_cat_file(test_repo: tempfile::TempDir) {
        let repo = Repo::new(test_repo.path());
        let mut stdout = Vec::new();

        good_git::cat_file(
            &repo,
            "d670460b4b4aece5915caf5c68d12f560a9fe3e4",
            &mut stdout,
        )
        .unwrap();
        assert_eq!(stdout, b"test content\n\n");
    }

    #[rstest]
    #[case("d670", b"test content\n\n".to_vec())]
    #[case("d67046", b"test content\n\n".to_vec())]
    #[case("1234567890", b"more content\nfrom a good client\n".to_vec())]
    fn test_cat_file_short_hash(
        test_repo: tempfile::TempDir,
        #[case] input: String,
        #[case] expected: Vec<u8>,
    ) {
        let repo = Repo::new(test_repo.path());
        let mut stdout = Vec::new();

        good_git::cat_file(&repo, &input, &mut stdout).unwrap();
        assert_eq!(stdout, expected);
    }

    #[rstest]
    #[case("")]
    #[case("d")]
    #[case("d6")]
    #[case("hello")]
    fn test_cat_file_fails_if_rev_not_found(test_repo: tempfile::TempDir, #[case] input: String) {
        let repo = Repo::new(test_repo.path());
        let mut stdout = Vec::new();

        let err = good_git::cat_file(&repo, &input, &mut stdout)
            .unwrap_err()
            .to_string();
        assert_eq!(err, "Object not found");
    }

    #[rstest]
    fn test_cat_file_blobs_and_trees(test_repo: tempfile::TempDir) {
        let repo = Repo::new(test_repo.path());
        let mut stdout = Vec::new();

        good_git::cat_file(
            &repo,
            "1234567890abcdef1234567890abcdef12345678",
            &mut stdout,
        )
        .unwrap();

        good_git::cat_file(
            &repo,
            "99887766554433221100aabbccddeeff00112233",
            &mut stdout,
        )
        .unwrap();

        assert_eq!(
            std::str::from_utf8(&stdout).unwrap(),
            "\
more content
from a good client
100644 blob d670460b4b4aece5915caf5c68d12f560a9fe3e4    test.txt
100644 blob 1234567890abcdef1234567890abcdef12345678    more.txt
"
        );
    }

    #[rstest]
    fn test_cat_file_commit(test_repo: tempfile::TempDir) {
        let repo = Repo::new(test_repo.path());
        let mut stdout = Vec::new();

        good_git::cat_file(
            &repo,
            "aaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbbbbb",
            &mut stdout,
        )
        .unwrap();
        assert_eq!(
            stdout,
            b"\
tree: 99887766554433221100aabbccddeeff00112233
parent: 
author: Bob <hello@bob.test>
committer: Alice <bye@alice.test>

This is a good commit
"
        );
    }

    #[rstest]
    fn test_log(test_repo: tempfile::TempDir) {
        let repo = Repo::new(test_repo.path());
        let mut stdout = Vec::new();

        good_git::log(
            &repo,
            "ccccccccccccccccccccdddddddddddddddddddd",
            &mut stdout,
        )
        .unwrap();
        assert_eq!(
            stdout,
            b"\
cccccc - Here is a better commit - \"Sherlock Holmes <sherlock@baker.street>\"
aaaaaa - This is a good commit - \"Alice <bye@alice.test>\"
",
        );
    }

    #[rstest]
    fn test_hash_object_w(test_repo: tempfile::TempDir) {
        // From https://git-scm.com/book/sv/v2/Git-Internals-Git-Objects
        let repo = Repo::new(test_repo.path());
        let mut stdout = Vec::new();

        good_git::hash_object(
            good_git::HashObjectMode::Write(&repo),
            &mut "test content\n".as_bytes(),
            &mut stdout,
        )
        .unwrap();

        const EXPECTED_HASH: &str = "d670460b4b4aece5915caf5c68d12f560a9fe3e4\n";
        assert_eq!(stdout, EXPECTED_HASH.as_bytes());
        stdout.clear();

        good_git::cat_file(&repo, EXPECTED_HASH.trim(), &mut stdout).unwrap();

        assert_eq!(stdout, b"test content\n\n");
    }
}
