//! SHA-256, for matching files against the known implant hashes in
//! `ioc/hashes.txt`.
//!
//! Written out rather than pulled from a crate. This is the cleanup tool for a
//! package supply-chain campaign, and "we added a dependency" is a poor look
//! on the one binary somebody runs on a machine they already distrust.
//!
//! Hand-writing a hash is only defensible because SHA-256 is fully specified
//! and has published test vectors: correctness here is demonstrated, not
//! asserted. The tests below run the NIST examples, the empty string, a
//! multi-block input and the million-`a` vector. A wrong implementation would
//! produce a missed detection, and those vectors are what stop that shipping.

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Streaming SHA-256, so a 300 MB file is hashed without being held in memory.
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    length: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: H0,
            buffer: [0u8; 64],
            buffered: 0,
            length: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.length = self.length.wrapping_add(data.len() as u64);

        if self.buffered > 0 {
            let need = 64 - self.buffered;
            let take = need.min(data.len());
            if let Some(slot) = self.buffer.get_mut(self.buffered..self.buffered + take) {
                slot.copy_from_slice(data.get(..take).unwrap_or_default());
            }
            self.buffered += take;
            data = data.get(take..).unwrap_or_default();
            if self.buffered == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffered = 0;
            }
        }

        while data.len() >= 64 {
            let (block, rest) = data.split_at(64);
            let mut b = [0u8; 64];
            b.copy_from_slice(block);
            self.compress(&b);
            data = rest;
        }

        if !data.is_empty() {
            if let Some(slot) = self.buffer.get_mut(..data.len()) {
                slot.copy_from_slice(data);
            }
            self.buffered = data.len();
        }
    }

    pub fn finish(mut self) -> [u8; 32] {
        let bits = self.length.wrapping_mul(8);

        // Padding: a single 1 bit, zeros, then the length as 64-bit big endian.
        self.update_raw(&[0x80]);
        while self.buffered != 56 {
            self.update_raw(&[0x00]);
        }
        let len_be = bits.to_be_bytes();
        self.update_raw(&len_be);

        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            let bytes = word.to_be_bytes();
            if let Some(slot) = out.get_mut(i * 4..i * 4 + 4) {
                slot.copy_from_slice(&bytes);
            }
        }
        out
    }

    /// Feed bytes without counting them towards the length, for padding.
    fn update_raw(&mut self, data: &[u8]) {
        for byte in data {
            if let Some(slot) = self.buffer.get_mut(self.buffered) {
                *slot = *byte;
            }
            self.buffered += 1;
            if self.buffered == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffered = 0;
            }
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            let b = block.get(i * 4..i * 4 + 4).unwrap_or(&[0; 4]);
            let mut v = [0u8; 4];
            v.copy_from_slice(b);
            if let Some(slot) = w.get_mut(i) {
                *slot = u32::from_be_bytes(v);
            }
        }
        for i in 16..64 {
            // Read every input before touching w mutably: a closure capturing
            // w immutably cannot coexist with the write below.
            let w15 = w.get(i - 15).copied().unwrap_or(0);
            let w2 = w.get(i - 2).copied().unwrap_or(0);
            let w16 = w.get(i - 16).copied().unwrap_or(0);
            let w7 = w.get(i - 7).copied().unwrap_or(0);
            let s0 = w15.rotate_right(7) ^ w15.rotate_right(18) ^ (w15 >> 3);
            let s1 = w2.rotate_right(17) ^ w2.rotate_right(19) ^ (w2 >> 10);
            if let Some(slot) = w.get_mut(i) {
                *slot = w16.wrapping_add(s0).wrapping_add(w7).wrapping_add(s1);
            }
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K.get(i).copied().unwrap_or(0))
                .wrapping_add(w.get(i).copied().unwrap_or(0));
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }

        for (slot, v) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(v);
        }
    }
}

/// Lowercase hex, the form `ioc/hashes.txt` uses.
pub fn hex(digest: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Hash a file without reading it all into memory.
pub fn file(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(buf.get(..n).unwrap_or_default());
    }
    Ok(hex(&h.finish()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn digest(s: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(s);
        hex(&h.finish())
    }

    /// The published vectors. A wrong implementation here is a missed
    /// detection, so correctness is demonstrated rather than assumed.
    #[test]
    fn nist_vectors() {
        assert_eq!(
            digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            digest(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn the_million_a_vector() {
        // Exercises multi-block streaming and the length counter past 2^20.
        let mut h = Sha256::new();
        let chunk = vec![b'a'; 1000];
        for _ in 0..1000 {
            h.update(&chunk);
        }
        assert_eq!(
            hex(&h.finish()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn a_block_boundary_is_handled() {
        // 55, 56 and 64 bytes are where the padding logic goes wrong if it is
        // going to: 56 forces an extra block for the length field.
        for n in [55usize, 56, 63, 64, 65] {
            let data = vec![b'x'; n];
            let mut streamed = Sha256::new();
            for b in &data {
                streamed.update(std::slice::from_ref(b));
            }
            assert_eq!(
                hex(&streamed.finish()),
                digest(&data),
                "byte-at-a-time and all-at-once must agree at {n} bytes"
            );
        }
    }
}
