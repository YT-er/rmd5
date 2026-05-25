use ::md5::{Digest, Md5 as InnerMd5};

pub struct Md5 {
    inner: InnerMd5,
}

impl Md5 {
    pub fn new() -> Self {
        Self {
            inner: InnerMd5::new(),
        }
    }

    pub fn update(&mut self, input: &[u8]) {
        self.inner.update(input);
    }

    pub fn finalize(self) -> [u8; 16] {
        let digest = self.inner.finalize();
        let mut out = [0u8; 16];
        out.copy_from_slice(&digest);
        out
    }
}

pub fn to_hex(digest: &[u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(32);
    for &byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub fn parse_hex(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }

    let mut out = [0u8; 16];
    for (i, pair) in s.as_bytes().chunks_exact(2).enumerate() {
        let hi = hex_value(pair[0])?;
        let lo = hex_value(pair[1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Md5, to_hex};

    #[test]
    fn known_vectors() {
        let cases = [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                "abcdefghijklmnopqrstuvwxyz",
                "c3fcd3d76192e4007dfb496cca67e13b",
            ),
        ];

        for (input, expected) in cases {
            let mut md5 = Md5::new();
            md5.update(input.as_bytes());
            assert_eq!(to_hex(&md5.finalize()), expected);
        }
    }
}
