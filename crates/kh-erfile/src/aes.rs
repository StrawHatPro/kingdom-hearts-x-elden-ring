//! AES-128 decryption, ECB (the archive's encrypted ranges). The S-boxes are computed once from the field's arithmetic (no
//! typed tables to get wrong); checked against the archive itself (a decrypted DCX header) and FIPS-197's example.

use std::sync::OnceLock;

fn xtime(b: u8) -> u8 {
    (b << 1) ^ if b & 0x80 != 0 { 0x1b } else { 0 }
}

fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0;
    while b != 0 {
        if b & 1 != 0 {
            p ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    p
}

struct Boxes {
    s: [u8; 256],
    inv: [u8; 256],
}

fn boxes() -> &'static Boxes {
    static B: OnceLock<Boxes> = OnceLock::new();
    B.get_or_init(|| {
        let mut s = [0u8; 256];
        let mut inv = [0u8; 256];
        for i in 0..256usize {
            let x = if i == 0 { 0 } else { (1..256u16).find(|j| gmul(i as u8, *j as u8) == 1).unwrap_or(0) as u8 };
            let y = x ^ x.rotate_left(1) ^ x.rotate_left(2) ^ x.rotate_left(3) ^ x.rotate_left(4) ^ 0x63;
            s[i] = y;
            inv[y as usize] = i as u8;
        }
        Boxes { s, inv }
    })
}

/// An expanded AES-128 key (11 round keys).
pub struct Aes128 {
    rk: [[u8; 16]; 11],
}

impl Aes128 {
    pub fn new(key: &[u8; 16]) -> Self {
        let s = &boxes().s;
        let mut w = [[0u8; 4]; 44];
        for i in 0..4 {
            w[i].copy_from_slice(&key[i * 4..i * 4 + 4]);
        }
        let mut rcon = 1u8;
        for i in 4..44 {
            let mut t = w[i - 1];
            if i % 4 == 0 {
                t = [s[t[1] as usize] ^ rcon, s[t[2] as usize], s[t[3] as usize], s[t[0] as usize]];
                rcon = xtime(rcon);
            }
            for k in 0..4 {
                w[i][k] = w[i - 4][k] ^ t[k];
            }
        }
        let mut rk = [[0u8; 16]; 11];
        for (r, key) in rk.iter_mut().enumerate() {
            for c in 0..4 {
                key[c * 4..c * 4 + 4].copy_from_slice(&w[r * 4 + c]);
            }
        }
        Aes128 { rk }
    }

    pub fn decrypt_block(&self, b: &mut [u8]) {
        let inv = &boxes().inv;
        let add = |b: &mut [u8], k: &[u8; 16]| b.iter_mut().zip(k).for_each(|(x, y)| *x ^= y);
        let unshift_unsub = |b: &mut [u8]| {
            let old: [u8; 16] = b[..16].try_into().unwrap();
            for c in 0..4 {
                for r in 0..4 {
                    b[r + 4 * ((c + r) % 4)] = inv[old[r + 4 * c] as usize];
                }
            }
        };
        add(b, &self.rk[10]);
        for round in (1..10).rev() {
            unshift_unsub(b);
            add(b, &self.rk[round]);
            for c in 0..4 {
                let a = [b[4 * c], b[4 * c + 1], b[4 * c + 2], b[4 * c + 3]];
                b[4 * c] = gmul(a[0], 14) ^ gmul(a[1], 11) ^ gmul(a[2], 13) ^ gmul(a[3], 9);
                b[4 * c + 1] = gmul(a[0], 9) ^ gmul(a[1], 14) ^ gmul(a[2], 11) ^ gmul(a[3], 13);
                b[4 * c + 2] = gmul(a[0], 13) ^ gmul(a[1], 9) ^ gmul(a[2], 14) ^ gmul(a[3], 11);
                b[4 * c + 3] = gmul(a[0], 11) ^ gmul(a[1], 13) ^ gmul(a[2], 9) ^ gmul(a[3], 14);
            }
        }
        unshift_unsub(b);
        add(b, &self.rk[0]);
    }

    /// ECB over whole 16-byte blocks of `data` (a tail shorter than a block is left as it is).
    pub fn decrypt_ecb(&self, data: &mut [u8]) {
        for block in data.chunks_exact_mut(16) {
            self.decrypt_block(block);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fips197_appendix_c1() {
        // key 000102..0f, plaintext 00112233..ff, ciphertext 69c4e0d86a7b0430d8cdb78070b4c55a
        let key: [u8; 16] = std::array::from_fn(|i| i as u8);
        let mut c = [0x69, 0xc4, 0xe0, 0xd8, 0x6a, 0x7b, 0x04, 0x30, 0xd8, 0xcd, 0xb7, 0x80, 0x70, 0xb4, 0xc5, 0x5a];
        Aes128::new(&key).decrypt_block(&mut c);
        let want: [u8; 16] = std::array::from_fn(|i| (i as u8) * 0x11);
        assert_eq!(c, want);
    }
}
