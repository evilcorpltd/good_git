use anyhow::{Context, Result, anyhow};
use flate2::read::ZlibDecoder;
use sha1::{Digest, Sha1};
use std::{fs, io::prelude::*};

use crate::repo::Repo;

#[derive(Debug)]
pub struct Blob {
    pub content: Vec<u8>,
}

impl Blob {
    pub fn new(content: Vec<u8>) -> Blob {
        Blob { content }
    }

    pub fn hash(&self) -> String {
        let size = self.content.len();
        let data = format!("blob {size}\0");
        let mut data = data.as_bytes().to_vec();
        data.extend(self.content.as_slice());

        hash(&data)
    }
}

#[derive(Debug)]
pub struct Tree {
    pub files: Vec<File>,
}

impl Tree {
    pub fn new(files: Vec<File>) -> Tree {
        Tree { files }
    }
}

#[derive(Debug, PartialEq)]
pub enum Mode {
    NormalFile,
    Executable,
    SymbolicLink,
    Tree,
    Submodule,
}

impl Mode {
    pub fn from_mode_str(mode: &str) -> Result<Mode> {
        match mode {
            "100644" => Ok(Mode::NormalFile),
            "100755" => Ok(Mode::Executable),
            "120000" => Ok(Mode::SymbolicLink),
            "40000" => Ok(Mode::Tree),
            "160000" => Ok(Mode::Submodule),
            _ => Err(anyhow!("Unknown mode")),
        }
    }

    pub fn mode_str(&self) -> &str {
        match self {
            Mode::NormalFile => "100644",
            Mode::Executable => "100755",
            Mode::SymbolicLink => "120000",
            Mode::Tree => "40000",
            Mode::Submodule => "160000",
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct File {
    pub mode: Mode,
    pub name: String,
    pub hash: String,
}

impl File {
    pub fn type_str(&self) -> &str {
        match self.mode {
            Mode::NormalFile => "blob",
            Mode::Executable => "blob",
            Mode::SymbolicLink => "symlink",
            Mode::Tree => "tree",
            Mode::Submodule => "submodule",
        }
    }
}

#[derive(Debug, Default)]
pub struct Commit {
    // Git seems to only consider the following standard headers:
    // https://github.com/git/git/blob/7b0defb3915eaa0bd118f0996e8c00b4eb2dc1ca/commit.c#L1442
    // TOOD: support merge commits.
    pub tree: String,
    pub parent: String,
    pub author: String,
    pub committer: String,
    pub encoding: String,

    pub message: String,
}

#[derive(Debug)]
pub enum Object {
    Blob(Blob),
    Tree(Tree),
    Commit(Commit),
}

impl Object {
    pub fn from_bytes(s: &[u8]) -> Result<Object> {
        let (object_type, object_size, header_end) = Object::parse_header(s)?;
        let mut content = &s[header_end + 1..];

        if content.len() != object_size {
            return Err(anyhow!("Incorrect header length"));
        }

        match object_type.as_str() {
            "blob" => {
                let blob = Blob::new(content.to_vec());
                Ok(Object::Blob(blob))
            }
            "tree" => {
                // Format (one per file/folder/tree/submodule):
                // [mode] [object name]\0[SHA-1 in binary format (20 bytes)]
                let mut files = vec![];
                while !content.is_empty() {
                    let mut mode = vec![];
                    let mode_size = content
                        .read_until(b' ', &mut mode)
                        .context("Failed to read mode")?;
                    let mode = Mode::from_mode_str(std::str::from_utf8(&mode[..mode_size - 1])?)
                        .context("Failed to parse mode")?;

                    let mut name = vec![];
                    let name_size = content
                        .read_until(b'\0', &mut name)
                        .context("Failed to read file name")?;
                    let name = std::str::from_utf8(&name[..name_size - 1])?;

                    let mut hash = [0_u8; 20];
                    content
                        .read_exact(&mut hash)
                        .context("Failed to read hash")?;
                    let hash = hex::encode(hash);

                    files.push(File {
                        mode,
                        name: name.to_string(),
                        hash,
                    });
                }
                let tree = Tree::new(files);
                Ok(Object::Tree(tree))
            }
            "commit" => {
                let content_str = std::str::from_utf8(content)?;
                let mut lines = content_str.lines();

                let mut commit = Commit::default();

                // Format is:
                // [key] [value]
                // ...
                // <empty line>
                // [commit message]
                while let Some(line) = lines.next() {
                    if line.is_empty() {
                        // End of commit header, everything after is the commit message
                        let value = lines.collect::<Vec<_>>().join("\n");
                        commit.message = value;
                        break;
                    }
                    let (key, value) = line.split_once(' ').ok_or(anyhow!("Invalid line"))?;
                    let value = value.to_string();
                    if key == "tree" {
                        commit.tree = value;
                    } else if key == "parent" {
                        commit.parent = value;
                    } else if key == "author" {
                        commit.author = value;
                    } else if key == "committer" {
                        commit.committer = value;
                    } else if key == "encoding" {
                        commit.encoding = value;
                    } else {
                        // TODO: unknown key. Should we handle it?
                    }
                }

                Ok(Object::Commit(commit))
            }
            _ => Err(anyhow!("Unknown object type")),
        }
    }

    pub fn from_file(path: &std::path::Path) -> Result<Object> {
        let data = std::fs::read(path).context("Could not read from file")?;
        let mut z = ZlibDecoder::new(&data[..]);
        let mut s: Vec<u8> = vec![];
        z.read_to_end(&mut s)?;

        Object::from_bytes(&s)
    }

    /// Returns an object from a hash in a git repository.
    pub fn from_hash(repo: &Repo, hash: &str) -> Result<Object> {
        let (short_hash, long_hash) = hash.split_at_checked(2).ok_or(anyhow!("Invalid hash"))?;
        let path = repo
            .git_dir()
            .join("objects")
            .join(short_hash)
            .join(long_hash);
        Object::from_file(&path)
    }

    /// Returns an object from a rev in a git repository.
    ///
    /// A rev can be a hash (long or short), a branch or a tag.
    /// If no matches are found, an error is returned.
    /// And error is also returned if the rev is ambiguous.
    pub fn from_rev(repo: &Repo, rev: &str) -> Result<Object> {
        let mut candidates: Vec<String> = vec![];

        // Check if this is a hash
        if rev.len() >= 4 {
            let (short_hash, long_hash) = rev.split_at(2);
            let path = repo.git_dir().join("objects").join(short_hash);

            if path.exists() {
                for entry in fs::read_dir(path)? {
                    let curr_path = entry?.path();
                    if let Some(file_name) = curr_path.file_name() {
                        if let Some(file_name_str) = file_name.to_str() {
                            if file_name_str.starts_with(long_hash) {
                                candidates.push(format!("{}{}", short_hash, file_name_str));
                            }
                        }
                    }
                }
            }
        }

        // TODO: Check if this is a branch or a tag

        match candidates.len() {
            1 => Ok(Object::from_hash(repo, &candidates[0])?),
            0 => Err(anyhow!("Object not found")),
            _ => Err(anyhow!("Ambiguous reference: {:?}", candidates)),
        }
    }

    /// Parse the header of a git object.
    ///
    /// The header is in the format: [object type] [object size]\0
    ///
    /// Returns the type, object size and the index where the header ends.
    fn parse_header(s: &[u8]) -> Result<(String, usize, usize)> {
        let space_index = s
            .iter()
            .position(|&x| x == b' ')
            .ok_or(anyhow!("Incorrect header format"))?;
        let null_index = s
            .iter()
            .position(|&x| x == b'\0')
            .ok_or(anyhow!("Incorrect header format"))?;
        let object_type = std::str::from_utf8(&s[..space_index])?;
        let object_size = std::str::from_utf8(&s[space_index + 1..null_index])?;
        let object_size = object_size.parse::<usize>()?;
        Ok((object_type.to_string(), object_size, null_index))
    }
}

pub fn hash(s: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(s);

    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use crate::object::File;

    use super::Blob;
    use super::Mode;
    use super::Object;
    use super::hash;
    #[test]
    fn test_object_parse_header() {
        assert_eq!(
            Object::parse_header(b"blob 16\0").unwrap(),
            ("blob".to_string(), 16, 7)
        );
    }

    #[test]
    fn test_object_parse_header_incorrect_format() {
        assert_eq!(
            Object::parse_header(b"blob 16").unwrap_err().to_string(),
            "Incorrect header format"
        );
        assert_eq!(
            Object::parse_header(b"blob").unwrap_err().to_string(),
            "Incorrect header format"
        );
    }

    #[test]
    fn test_object_from_bytes_for_blob() {
        let s = b"blob 16\0what is up, doc?";
        let object = Object::from_bytes(s.as_ref()).unwrap();
        let Object::Blob(blob) = object else {
            panic!("Expected a Blob");
        };
        assert_eq!(blob.content, b"what is up, doc?");
    }

    #[test]
    fn test_object_from_bytes_for_tree() {
        let s = b"tree 107\0\
            100644 file1.txt\0\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\
            100644 file2.txt\0\x51\x52\x53\x54\x55\x56\x57\x58\x59\x5a\x5b\x5c\x5d\x5e\x5f\x60\x61\x62\x63\x64\
            40000 folder\0\x81\x82\x83\x84\x85\x86\x87\x88\x89\x8a\x8b\x8c\x8d\x8e\x8f\x90\x91\x92\x93\x94";
        let object = Object::from_bytes(s.as_ref()).unwrap();
        let Object::Tree(tree) = object else {
            panic!("Expected a tree");
        };
        assert_eq!(
            tree.files,
            vec![
                File {
                    mode: Mode::NormalFile,
                    name: "file1.txt".to_string(),
                    hash: "0102030405060708090a0b0c0d0e0f1011121314".to_string(),
                },
                File {
                    mode: Mode::NormalFile,
                    name: "file2.txt".to_string(),
                    hash: "5152535455565758595a5b5c5d5e5f6061626364".to_string(),
                },
                File {
                    mode: Mode::Tree,
                    name: "folder".to_string(),
                    hash: "8182838485868788898a8b8c8d8e8f9091929394".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_object_from_bytes_for_commit() {
        let s = b"commit 118\0\
tree abc123
parent 987xyz
author good_git <good@git.com> 1234 +0100

Add good git

This commit adds a good git client
";
        let object = Object::from_bytes(s.as_ref()).unwrap();
        let Object::Commit(commit) = object else {
            panic!("Expected a commit");
        };
        assert_eq!(commit.tree, "abc123");
        assert_eq!(commit.parent, "987xyz");
        assert_eq!(commit.author, "good_git <good@git.com> 1234 +0100");
        assert_eq!(commit.committer, "");
        assert_eq!(
            commit.message,
            "Add good git\n\nThis commit adds a good git client"
        );
    }

    #[test]
    fn test_object_from_bytes_for_commit_with_incorrect_format() {
        let s = b"commit 18\0\
tree abc123
parent";
        let err = Object::from_bytes(s.as_ref()).unwrap_err().to_string();
        assert_eq!(err, "Invalid line");
    }

    #[test]
    fn test_object_from_bytes_for_tree_incorrect_hash_length() {
        let s = b"tree 18\0\
            100644 file1.txt\0\x01";
        let err = Object::from_bytes(s.as_ref()).unwrap_err().to_string();
        assert_eq!(err, "Failed to read hash");
    }

    #[test]
    fn test_object_from_bytes_for_tree_invalid_mode() {
        let s = b"tree 7\0\
            123456 ";
        let err = Object::from_bytes(s.as_ref()).unwrap_err().to_string();
        assert_eq!(err, "Failed to parse mode");
    }

    #[test]
    fn test_object_from_bytes_incorrect_header_size() {
        let s = b"blob 0\0hi";
        let err = Object::from_bytes(s.as_ref()).unwrap_err().to_string();
        assert_eq!(err, "Incorrect header length");
    }

    #[test]
    fn test_blob_hash_is_correct() {
        // From https://git-scm.com/book/sv/v2/Git-Internals-Git-Objects
        let blob = Blob::new(b"what is up, doc?".to_vec());
        assert_eq!(blob.hash(), "bd9dbf5aae1a3862dd1526723246b20206e5fc37");
    }

    #[test]
    fn test_hash_is_correct() {
        // From https://git-scm.com/book/sv/v2/Git-Internals-Git-Objects
        let s = b"blob 16\0what is up, doc?";
        assert_eq!(hash(s), "bd9dbf5aae1a3862dd1526723246b20206e5fc37");
    }
}
