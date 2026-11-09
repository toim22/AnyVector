//! PDF Standard security handler, revision 3 (128-bit RC4), used by Publish to PDF ▸ Settings ▸
//! Security. MD5 and RC4 are small enough to carry here.

const PAD: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08, 0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C, 0xA9, 0xFE, 0x64,
    0x53, 0x69, 0x7A,
];

pub fn md5(input: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64).map(|i| ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32).collect();
    let (mut a0, mut b0, mut c0, mut d0) = (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    let mut msg = input.to_vec();
    let bits = (input.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_le_bytes());
    for chunk in msg.chunks(64) {
        let m: Vec<u32> = (0..16).map(|i| u32::from_le_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]])).collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    for (i, v) in [a0, b0, c0, d0].iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    out
}

pub fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut s: [u8; 256] = core::array::from_fn(|i| i as u8);
    let mut j = 0u8;
    for i in 0..256 {
        j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
        s.swap(i, j as usize);
    }
    let (mut i, mut j) = (0u8, 0u8);
    data.iter()
        .map(|b| {
            i = i.wrapping_add(1);
            j = j.wrapping_add(s[i as usize]);
            s.swap(i as usize, j as usize);
            b ^ s[s[i as usize].wrapping_add(s[j as usize]) as usize]
        })
        .collect()
}

fn padded(pw: &str) -> [u8; 32] {
    let bytes: Vec<u8> = pw.chars().map(|c| if (c as u32) < 256 { c as u32 as u8 } else { b'?' }).take(32).collect();
    let mut out = PAD;
    out[..bytes.len()].copy_from_slice(&bytes);
    out[bytes.len()..].copy_from_slice(&PAD[..32 - bytes.len()]);
    out
}

fn rc4_rounds(key: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let mut out = rc4(key, data);
    for i in 1..=19u8 {
        let k: Vec<u8> = key.iter().map(|b| b ^ i).collect();
        out = rc4(&k, &out);
    }
    out
}

/// Printing / editing / copying permissions for the /P entry.
pub fn permissions(printing: u8, editing: u8, copying: bool) -> i32 {
    let bit = |n: u32| 1u32 << (n - 1);
    let mask = bit(3) | bit(4) | bit(5) | bit(6) | bit(9) | bit(10) | bit(11) | bit(12);
    let mut p = !0u32 & !3 & !mask;
    match printing {
        1 => p |= bit(3),
        2 => p |= bit(3) | bit(12),
        _ => {}
    }
    match editing {
        1 => p |= bit(11),
        2 => p |= bit(4) | bit(6) | bit(9) | bit(11),
        _ => {}
    }
    if copying {
        p |= bit(5) | bit(10);
    }
    p as i32
}

pub struct Crypt {
    key: [u8; 16],
    pub o: [u8; 32],
    pub u: [u8; 32],
    pub p: i32,
    pub id: [u8; 16],
}

impl Crypt {
    pub fn new(user: &str, owner: &str, p: i32, id: [u8; 16]) -> Crypt {
        let owner = if owner.is_empty() { user } else { owner };
        // Algorithm 3: the owner entry
        let mut h = md5(&padded(owner));
        for _ in 0..50 {
            h = md5(&h);
        }
        let o_vec = rc4_rounds(&h, &padded(user));
        let mut o = [0u8; 32];
        o.copy_from_slice(&o_vec);
        // Algorithm 2: the file key
        let mut input = padded(user).to_vec();
        input.extend_from_slice(&o);
        input.extend_from_slice(&(p as u32).to_le_bytes());
        input.extend_from_slice(&id);
        let mut key = md5(&input);
        for _ in 0..50 {
            key = md5(&key);
        }
        // Algorithm 5: the user entry
        let mut input = PAD.to_vec();
        input.extend_from_slice(&id);
        let u16b = rc4_rounds(&key, &md5(&input));
        let mut u = [0u8; 32];
        u[..16].copy_from_slice(&u16b);
        u[16..].copy_from_slice(&PAD[..16]);
        Crypt { key, o, u, p, id }
    }

    pub fn encrypt(&self, num: usize, data: &[u8]) -> Vec<u8> {
        let mut k = self.key.to_vec();
        k.extend_from_slice(&[(num & 0xff) as u8, ((num >> 8) & 0xff) as u8, ((num >> 16) & 0xff) as u8, 0, 0]);
        rc4(&md5(&k), data)
    }

    pub fn dict(&self) -> String {
        format!("<< /Filter /Standard /V 2 /R 3 /Length 128 /O <{}> /U <{}> /P {} >>", hex(&self.o), hex(&self.u), self.p)
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        assert_eq!(hex(&md5(b"")), "D41D8CD98F00B204E9800998ECF8427E");
        assert_eq!(hex(&md5(b"The quick brown fox jumps over the lazy dog")), "9E107D9D372BB6826BD81D3542A419D6");
        assert_eq!(hex(&rc4(b"Key", b"Plaintext")), "BBF316E8D940AF0AD3");
        assert_eq!(hex(&rc4(b"Wiki", b"pedia")), "1021BF0420");
        // all permissions = -4 with the reserved high bits set
        assert_eq!(permissions(2, 2, true), -3904 | (4 | 8 | 16 | 32 | 256 | 512 | 1024 | 2048));
        assert_eq!(permissions(0, 0, false), -3904);
    }
}
