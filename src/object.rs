use sha1::{Digest, Sha1};

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

pub fn hash(s: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(s);

    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::hash;
    use super::Blob;

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
