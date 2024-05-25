use anyhow::{anyhow, Context, Result};
use flate2::read::ZlibDecoder;
use sha1::{Digest, Sha1};
use std::io::prelude::*;

#[derive(Debug)]
pub struct Blob {
    pub content: Vec<u8>,
}

impl Blob {
    pub fn new(content: Vec<u8>) -> Blob {
        Blob { content }
    }

    pub fn hash(self) -> String {
        let size = self.content.len();
        let data = format!("blob {size}\0");
        let mut data = data.as_bytes().to_vec();
        data.extend(self.content);

        hash(&data)
    }
}

#[derive(Debug)]
pub enum Object {
    Blob(Blob),
}

impl Object {
    pub fn from_bytes(s: &[u8]) -> Result<Object> {
        let (object_type, object_size, header_end) = Object::parse_header(s)?;
        let content = &s[header_end + 1..];

        if content.len() != object_size {
            return Err(anyhow!("Incorrect header length"));
        }

        match object_type.as_str() {
            "blob" => {
                let blob = Blob::new(content.to_vec());
                Ok(Object::Blob(blob))
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
    use super::hash;
    use super::Blob;
    use super::Object;
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
    fn test_blob_from_bytes() {
        let s = b"blob 16\0what is up, doc?";
        let object = Object::from_bytes(s.as_ref()).unwrap();
        let Object::Blob(blob) = object;
        assert_eq!(blob.content, b"what is up, doc?");
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
