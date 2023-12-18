use base64::{engine::general_purpose, Engine};
use md5::{Digest, Md5};
use sha1::Sha1;
use sha2::Sha256;

/// Calc md5 digest
pub fn md5(input: impl AsRef<[u8]>) -> String {
    let mut md5 = Md5::new();
    md5.update(input);
    hex::encode(md5.finalize())
}

/// Calc md5 digest
#[allow(unused)]
pub fn md5_vec(input: &[impl AsRef<[u8]>]) -> String {
    let mut md5 = Md5::new();
    for p in input {
        md5.update(p);
    }
    hex::encode(md5.finalize())
}

/// Calc sha1 digest
pub fn sha1(input: impl AsRef<[u8]>) -> String {
    let mut sha1 = Sha1::new();
    sha1.update(input);
    hex::encode(sha1.finalize())
}

/// Calc sha1 digest
#[allow(unused)]
pub fn sha1_vec(input: &[impl AsRef<[u8]>]) -> String {
    let mut sha1 = Sha1::new();
    for i in input {
        sha1.update(i);
    }
    hex::encode(sha1.finalize())
}

/// Calc sha256 digest
pub fn sha256(input: impl AsRef<[u8]>) -> String {
    let mut sha256 = Sha256::new();
    sha256.update(input);
    hex::encode(sha256.finalize())
}

/// Calc base64
pub fn encode_base64(input: impl AsRef<[u8]>) -> String {
    general_purpose::STANDARD.encode(input)
}

#[cfg(test)]
mod tests {
    use crate::digest::*;

    #[test]
    fn test_md5() {
        let input = "INPUT";
        let output = md5(input);
        assert_eq!("a84cc046d48610b05c21fd3670d0c829", output);
    }

    #[test]
    fn test_md5_vec() {
        let input = vec!["I", "N", "P", "U", "T"];
        let output = md5_vec(&input);
        assert_eq!("a84cc046d48610b05c21fd3670d0c829", output);
    }

    #[test]
    fn test_sha1() {
        let input = "INPUT";
        let output = sha1(input);
        assert_eq!("bb2fe63e5a32cb2596d9f60d2ae271ae4d1c1787", output);
    }

    #[test]
    fn test_sha1_vec() {
        let input = vec!["I", "N", "P", "U", "T"];
        let output = sha1_vec(&input);
        assert_eq!("bb2fe63e5a32cb2596d9f60d2ae271ae4d1c1787", output);
    }

    #[test]
    fn test_sha256() {
        let input = "INPUT";
        let output = sha256(input);
        assert_eq!(
            "f4262548cb993257ce8409eec8b0382e2836b5dd6d9cec1e8527b458dccd3098",
            output
        );
    }
}
