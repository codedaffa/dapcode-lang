//! Pustaka Standar Kriptografi dan Encoding DapCode
//!
//! Modul ini menyediakan implementasi murni tanpa dependensi eksternal untuk:
//! - SHA-256 (FIPS 180-4)
//! - MD5 (RFC 1321)
//! - Base64 Encoding & Decoding (RFC 4648)

// =========================================================================
// 1. SHA-256 (FIPS 180-4)
// =========================================================================

const K_SHA256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256(input: &str) -> String {
    let bytes = input.as_bytes();
    let bit_len = (bytes.len() as u64) * 8;

    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut h0: u32 = 0x6a09e667;
    let mut h1: u32 = 0xbb67ae85;
    let mut h2: u32 = 0x3c6ef372;
    let mut h3: u32 = 0xa54ff53a;
    let mut h4: u32 = 0x510e527f;
    let mut h5: u32 = 0x9b05688c;
    let mut h6: u32 = 0x1f83d9ab;
    let mut h7: u32 = 0x5be0cd19;

    for chunk in padded.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;
        let mut f = h5;
        let mut g = h6;
        let mut h = h7;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K_SHA256[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
        h5 = h5.wrapping_add(f);
        h6 = h6.wrapping_add(g);
        h7 = h7.wrapping_add(h);
    }

    format!(
        "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        h0, h1, h2, h3, h4, h5, h6, h7
    )
}

// =========================================================================
// 2. MD5 (RFC 1321)
// =========================================================================

pub fn md5(input: &str) -> String {
    let bytes = input.as_bytes();
    let bit_len = (bytes.len() as u64) * 8;

    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    let s: [u32; 64] = [
        7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,  7, 12, 17, 22,
        5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,  5,  9, 14, 20,
        4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,  4, 11, 16, 23,
        6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21,  6, 10, 15, 21,
    ];

    let mut k = [0u32; 64];
    for i in 0..64 {
        let val = ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u64;
        k[i] = val as u32;
    }

    for chunk in padded.chunks(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }

        let mut a = a0;
        let mut b = b0;
        let mut c = c0;
        let mut d = d0;

        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | (!d)), (7 * i) % 16),
            };

            let temp = d;
            d = c;
            c = b;
            let sum = a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(s[i]));
            a = temp;
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let a_bytes = a0.to_le_bytes();
    let b_bytes = b0.to_le_bytes();
    let c_bytes = c0.to_le_bytes();
    let d_bytes = d0.to_le_bytes();

    format!(
        "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        a_bytes[0], a_bytes[1], a_bytes[2], a_bytes[3],
        b_bytes[0], b_bytes[1], b_bytes[2], b_bytes[3],
        c_bytes[0], c_bytes[1], c_bytes[2], c_bytes[3],
        d_bytes[0], d_bytes[1], d_bytes[2], d_bytes[3]
    )
}

// =========================================================================
// 3. BASE64 (RFC 4648)
// =========================================================================

const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn ke_base64(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);

    for chunk in bytes.chunks(3) {
        match chunk.len() {
            3 => {
                let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
                out.push(B64_CHARS[((n >> 18) & 63) as usize] as char);
                out.push(B64_CHARS[((n >> 12) & 63) as usize] as char);
                out.push(B64_CHARS[((n >> 6) & 63) as usize] as char);
                out.push(B64_CHARS[(n & 63) as usize] as char);
            }
            2 => {
                let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8);
                out.push(B64_CHARS[((n >> 18) & 63) as usize] as char);
                out.push(B64_CHARS[((n >> 12) & 63) as usize] as char);
                out.push(B64_CHARS[((n >> 6) & 63) as usize] as char);
                out.push('=');
            }
            1 => {
                let n = (chunk[0] as u32) << 16;
                out.push(B64_CHARS[((n >> 18) & 63) as usize] as char);
                out.push(B64_CHARS[((n >> 12) & 63) as usize] as char);
                out.push('=');
                out.push('=');
            }
            _ => unreachable!(),
        }
    }

    out
}

pub fn dari_base64(input: &str) -> Result<String, String> {
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 4 != 0 {
        return Err("Panjang teks Base64 harus kelipatan 4".to_string());
    }

    let decode_char = |c: char| -> Result<u8, String> {
        match c {
            'A'..='Z' => Ok(c as u8 - b'A'),
            'a'..='z' => Ok(c as u8 - b'a' + 26),
            '0'..='9' => Ok(c as u8 - b'0' + 52),
            '+' => Ok(62),
            '/' => Ok(63),
            '=' => Ok(0),
            _ => Err(format!("Karakter Base64 tidak valid: '{}'", c)),
        }
    };

    let mut out_bytes = Vec::new();
    let chars: Vec<char> = clean.chars().collect();

    for chunk in chars.chunks(4) {
        let b0 = decode_char(chunk[0])?;
        let b1 = decode_char(chunk[1])?;
        let b2 = decode_char(chunk[2])?;
        let b3 = decode_char(chunk[3])?;

        let n = ((b0 as u32) << 18) | ((b1 as u32) << 12) | ((b2 as u32) << 6) | (b3 as u32);

        out_bytes.push(((n >> 16) & 0xFF) as u8);
        if chunk[2] != '=' {
            out_bytes.push(((n >> 8) & 0xFF) as u8);
        }
        if chunk[3] != '=' {
            out_bytes.push((n & 0xFF) as u8);
        }
    }

    String::from_utf8(out_bytes).map_err(|e| format!("Hasil decode bukan UTF-8 valid: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_known_vectors() {
        assert_eq!(
            sha256(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(
            sha256("DapCode 2026"),
            "30e9dfca33878fe64d3c0fb603e58200e1c4890d27f452506ec989cbf5f5820b"
        );
    }

    #[test]
    fn test_md5_known_vectors() {
        assert_eq!(md5(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5("hello"), "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn test_base64_roundtrip() {
        assert_eq!(ke_base64(""), "");
        assert_eq!(ke_base64("f"), "Zg==");
        assert_eq!(ke_base64("fo"), "Zm8=");
        assert_eq!(ke_base64("foo"), "Zm9v");
        assert_eq!(ke_base64("DapCode"), "RGFwQ29kZQ==");

        assert_eq!(dari_base64("RGFwQ29kZQ==").unwrap(), "DapCode");
        assert_eq!(dari_base64("Zm9v").unwrap(), "foo");
        assert!(dari_base64("invalid!").is_err());
    }
}
