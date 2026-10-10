use std::{collections::BTreeMap, fs};

use anyhow::{Result, anyhow};

use crate::repo::Repo;

const HEADER_SIZE: usize = 12;
const INDEX_ENTRY_HEADER_SIZE: usize = 62;
const CHECKSUM_SIZE: usize = 20;
const MAX_NAME_LEN: usize = 0xFFF;

/// An entry in the index, from <https://git-scm.com/docs/index-format>.
///
/// In the index file an entry has the following format (all numbers are network byte order):
///
///  0  32-bit ctime seconds
///  4  32-bit ctime nanosecond fractions
///  8  32-bit mtime seconds
/// 12  32-bit mtime nanosecond fractions
/// 16  32-bit dev
/// 20  32-bit ino
/// 24  32-bit mode, split into (high to low bits)
///       4-bit object type: 1000 (regular file), 1010 (symbolic link) or 1110 (gitlink)
///       3-bit unused
///       9-bit unix permission: 0755 or 0644 for regular files. 0 for symlinks and gitlinks
/// 28  32-bit uid
/// 32  32-bit gid
/// 36  32-bit file size
/// 40  sha-1 (20 bytes)
/// 60  16-bit flags, split into (high to low bits)
///       1-bit assume-valid flag
///       1-bit extended flag (must be zero in version 2)
///       2-bit stage (during merge)
///       12-bit name length if the length is less than 0xFFF otherwise 0xFFF
/// 62  entry path name (variable length)
///     relative to the top level directory without leading /
///     using / as path separator.
///     1-8 nul bytes as necessary to pad the entry to a multiple of eight bytes
///
/// Only the mode, sha-1 and entry path name are kept, the rest is skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexEntry {
    pub mode: u32,
    pub hash: String,
    pub path: String,
}

impl IndexEntry {
    /// Parses the entry at the start of data.
    ///
    /// Returns the entry and the size in bytes.
    fn from_bytes(data: &[u8]) -> Result<(IndexEntry, usize)> {
        let header = data
            .get(..INDEX_ENTRY_HEADER_SIZE)
            .ok_or_else(|| anyhow!("Index entry too short"))?;
        let mode = u32::from_be_bytes(header[24..28].try_into()?);
        let hash = hex::encode(&header[40..60]);
        let flags = u16::from_be_bytes(header[60..62].try_into()?);
        let name_len = flags as usize & MAX_NAME_LEN;
        if name_len == MAX_NAME_LEN {
            // TODO: support long paths.
            return Err(anyhow!(
                "Paths of {MAX_NAME_LEN} bytes or longer are not supported"
            ));
        }

        let path_end = INDEX_ENTRY_HEADER_SIZE + name_len;
        let path_data = data
            .get(INDEX_ENTRY_HEADER_SIZE..path_end)
            .ok_or_else(|| anyhow!("Incorrect path length"))?;
        let path = std::str::from_utf8(path_data)?.to_string();

        // At least one NUL, padding up to a multiple of 8 bytes.
        let size = (path_end + 1).next_multiple_of(8);
        Ok((IndexEntry { mode, hash, path }, size))
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Index {
    pub entries: BTreeMap<String, IndexEntry>,
}

impl Index {
    pub fn read(repo: &Repo) -> Result<Index> {
        let path = repo.git_dir().join("index");
        if !path.exists() {
            return Ok(Index::default());
        }
        Index::from_bytes(&fs::read(path)?)
    }

    /// Parses an index file, from <https://git-scm.com/docs/index-format>.
    /// Only version 2 is supported.
    ///
    /// The file has the following format (all numbers are network byte order):
    ///
    ///  0  4-byte signature: { 'D', 'I', 'R', 'C' }
    ///  4  4-byte version number
    ///  8  32-bit number of index entries
    /// 12  Index entries, see IndexEntry. Sorted in ascending order on the name field
    /// -   extensions
    /// -   sha-1 checksum (20 bytes)
    pub fn from_bytes(data: &[u8]) -> Result<Index> {
        if data.len() < HEADER_SIZE + CHECKSUM_SIZE {
            return Err(anyhow!("Index file too short"));
        }
        // TODO: Verify the checksum, the last CHECKSUM_SIZE bytes.
        let body = &data[..data.len() - CHECKSUM_SIZE];
        if &body[0..4] != b"DIRC" {
            return Err(anyhow!("Incorrect header signature"));
        }
        let version = u32::from_be_bytes(body[4..8].try_into()?);
        if version != 2 {
            return Err(anyhow!("Unsupported index version"));
        }

        let index_count = u32::from_be_bytes(body[8..12].try_into()?);

        let mut entries = BTreeMap::new();
        let mut data = &body[HEADER_SIZE..];
        for _ in 0..index_count {
            let (entry, size) = IndexEntry::from_bytes(data)?;
            data = data.get(size..).ok_or_else(|| anyhow!("Index too short"))?;
            entries.insert(entry.path.clone(), entry);
        }

        Ok(Index { entries })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn test_read_missing_index_is_empty() {
        let dir = tempdir().unwrap();
        let index = Index::read(&Repo::new(dir.path())).unwrap();
        assert_eq!(index, Index::default());
    }

    #[test]
    fn test_from_bytes_no_entries() {
        let data = hex::decode(
            [
                &hex::encode("DIRC"),                       // 0: signature
                "00000002",                                 // 4: version 2
                "00000000",                                 // 8: 0 entries
                "0000000000000000000000000000000000000000", // checksum
            ]
            .concat(),
        )
        .unwrap();

        assert_eq!(Index::from_bytes(&data).unwrap(), Index::default());
    }

    fn test_index() -> Vec<u8> {
        hex::decode(
            [
                // The header. The positions are from the start of the file.
                &hex::encode("DIRC"), // 0: "DIRC"
                "00000002",           // 4: version 2
                "00000003",           // 8: 3 entries
                // Entry "a", 1 byte path. The positions are from the start of the entry.
                "00000001",                                 // 0: ctime
                "00000002",                                 // 4: ctime nsec
                "00000003",                                 // 8: mtime
                "00000004",                                 // 12: mtime nsec
                "00000005",                                 // 16: dev
                "00000006",                                 // 20: ino
                "00000007",                                 // 24: mode
                "00000008",                                 // 28: uid
                "00000009",                                 // 32: gid
                "00000010",                                 // 36: size
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", // 40: sha-1
                "0001",                                     // 60: flags, path length 1
                &hex::encode("a"),                          // 62: "a"
                "00",                                       // 63: 1 NUL to make the entry 64 bytes
                // Entry "ab", 2 byte path.
                "00000000",                                 // 0: ctime
                "00000000",                                 // 4: ctime nsec
                "10000000",                                 // 8: mtime
                "20000000",                                 // 12: mtime nsec
                "30000000",                                 // 16: dev
                "40000000",                                 // 20: ino
                "50000000",                                 // 24: mode
                "60000000",                                 // 28: uid
                "70000000",                                 // 32: gid
                "80000000",                                 // 36: size
                "bbbbbbbbbbbbbbbbbbbbcccccccccccccccccccc", // 40: sha-1
                "0002",                                     // 60: flags, path length 2
                &hex::encode("ab"),                         // 62: "ab"
                "0000000000000000",                         // 64: 8 NULs to make the entry 72 bytes
                // Entry "dir/file.txt", 12 byte path.
                "00010000",                                 // 0: ctime
                "00020000",                                 // 4: ctime nsec
                "00030000",                                 // 8: mtime
                "00040000",                                 // 12: mtime nsec
                "00050000",                                 // 16: dev
                "00060000",                                 // 20: ino
                "00070000",                                 // 24: mode
                "00080000",                                 // 28: uid
                "00090000",                                 // 32: gid
                "00100000",                                 // 36: size
                "1234567890abcdef1234567890abcdef12345678", // 40: sha-1
                "000c",                                     // 60: flags, path length 12
                &hex::encode("dir/file.txt"),               // 62: "dir/file.txt"
                "000000000000",                             // 74: 6 NULs to make the entry 80 bytes
                // SHA-1 of everything.
                "0000000000000000000000000000000000000000",
            ]
            .concat(),
        )
        .unwrap()
    }

    fn create_entry(mode: u32, hash: &str, path: &str) -> (String, IndexEntry) {
        let entry = IndexEntry {
            mode,
            hash: hash.to_string(),
            path: path.to_string(),
        };
        (path.to_string(), entry)
    }

    #[test]
    fn test_from_bytes() {
        let expected = Index {
            entries: BTreeMap::from([
                create_entry(0x7, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "a"),
                create_entry(
                    0x5000_0000,
                    "bbbbbbbbbbbbbbbbbbbbcccccccccccccccccccc",
                    "ab",
                ),
                create_entry(
                    0x0007_0000,
                    "1234567890abcdef1234567890abcdef12345678",
                    "dir/file.txt",
                ),
            ]),
        };

        assert_eq!(Index::from_bytes(&test_index()).unwrap(), expected);
    }

    #[test]
    fn test_from_bytes_file_too_short() {
        assert_eq!(
            Index::from_bytes(b"hello").unwrap_err().to_string(),
            "Index file too short"
        );
    }

    #[test]
    fn test_from_bytes_wrong_header() {
        let data = hex::decode(
            [
                &hex::encode("DIRX"),                       // 0: wrong signature
                "00000002",                                 // 4: version 2
                "00000000",                                 // 8: 0 entries
                "0000000000000000000000000000000000000000", // checksum, all zeros
            ]
            .concat(),
        )
        .unwrap();

        assert_eq!(
            Index::from_bytes(&data).unwrap_err().to_string(),
            "Incorrect header signature"
        );
    }

    #[test]
    fn test_from_bytes_index_too_short() {
        let mut data = test_index();
        // Remove some bytes in the end
        data = data[..data.len() - 3].to_vec();

        assert_eq!(
            Index::from_bytes(&data).unwrap_err().to_string(),
            "Index too short"
        );
    }

    #[test]
    fn test_from_bytes_entry_too_short() {
        let mut data = test_index();
        // Remove some bytes in the end
        data = data[..data.len() - CHECKSUM_SIZE - 3].to_vec();

        assert_eq!(
            Index::from_bytes(&data).unwrap_err().to_string(),
            "Index entry too short"
        );
    }

    #[test]
    fn test_from_bytes_path_length_fff_not_supported() {
        let data = hex::decode(
            [
                &hex::encode("DIRC"),                       // 0: "DIRC"
                "00000002",                                 // 4: version 2
                "00000001",                                 // 8: 1 entry
                &"00000000".repeat(10), // 0: ctime, ctime nsec, mtime, mtime nsec, dev, ino, mode, uid, gid and size
                &"00".repeat(20),       // 40: sha-1
                "0fff",                 // 60: flags, path length 0xFFF
                &hex::encode("a".repeat(MAX_NAME_LEN)), // 62: path
                "000000",               // 3 NULs
                "0000000000000000000000000000000000000000", // checksum
            ]
            .concat(),
        )
        .unwrap();

        assert_eq!(
            Index::from_bytes(&data).unwrap_err().to_string(),
            "Paths of 4095 bytes or longer are not supported"
        );
    }
}
