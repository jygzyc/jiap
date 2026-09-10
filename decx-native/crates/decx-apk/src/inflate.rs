//! Pure-std DEFLATE (RFC 1951) decompressor. No compression side — APKs only need read.

pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize, // byte position
    bit: u32,   // bit buffer
    cnt: u32,   // bits in buffer
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0, bit: 0, cnt: 0 }
    }

    fn need(&mut self, n: u32) -> Result<(), String> {
        while self.cnt < n {
            let b = *self.data.get(self.pos).ok_or("deflate: out of input")?;
            self.pos += 1;
            self.bit |= (b as u32) << self.cnt;
            self.cnt += 8;
        }
        Ok(())
    }

    fn bits(&mut self, n: u32) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        self.need(n)?;
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.cnt -= n;
        Ok(v)
    }

    fn align(&mut self) {
        self.bit = 0;
        self.cnt = 0;
    }
}

/// Canonical Huffman decoding table (simple code-length based, no tree).
struct Huffman {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Huffman {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..16 {
            offs[i] = offs[i - 1] + counts[i - 1];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Huffman { counts, symbols }
    }

    fn decode(&self, br: &mut BitReader) -> Result<u16, String> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= br.bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err("deflate: bad huffman code".into())
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];

/// Inflate a raw DEFLATE stream into `out`.
pub fn inflate(data: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
    let mut br = BitReader::new(data);
    loop {
        let bfinal = br.bits(1)?;
        let btype = br.bits(2)?;
        match btype {
            0 => {
                // stored block
                br.align();
                let len = br.bits(16)? as usize;
                let nlen = br.bits(16)? as usize;
                if len != (!nlen & 0xFFFF) {
                    return Err("deflate: stored block length mismatch".into());
                }
                for _ in 0..len {
                    out.push(br.bits(8)? as u8);
                }
            }
            1 => {
                // fixed huffman
                let mut lit_lengths = [0u8; 288];
                for (i, l) in lit_lengths.iter_mut().enumerate() {
                    *l = if i < 144 {
                        8
                    } else if i < 256 {
                        9
                    } else if i < 280 {
                        7
                    } else {
                        8
                    };
                }
                let dist_lengths = [5u8; 30];
                let lit = Huffman::new(&lit_lengths);
                let dist = Huffman::new(&dist_lengths);
                inflate_block(&mut br, out, &lit, &dist)?;
            }
            2 => {
                // dynamic huffman
                let hlit = br.bits(5)? as usize + 257;
                let hdist = br.bits(5)? as usize + 1;
                let hclen = br.bits(4)? as usize + 4;
                const ORDER: [usize; 19] = [
                    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                ];
                let mut code_lengths = [0u8; 19];
                for i in 0..hclen {
                    code_lengths[ORDER[i]] = br.bits(3)? as u8;
                }
                let clh = Huffman::new(&code_lengths);
                let mut lengths = vec![0u8; hlit + hdist];
                let mut i = 0;
                while i < lengths.len() {
                    let sym = clh.decode(&mut br)?;
                    match sym {
                        0..=15 => {
                            lengths[i] = sym as u8;
                            i += 1;
                        }
                        16 => {
                            if i == 0 {
                                return Err("deflate: repeat with no previous".into());
                            }
                            let prev = lengths[i - 1];
                            let rep = 3 + br.bits(2)? as usize;
                            for _ in 0..rep {
                                if i >= lengths.len() {
                                    return Err("deflate: repeat overflow".into());
                                }
                                lengths[i] = prev;
                                i += 1;
                            }
                        }
                        17 => {
                            let rep = 3 + br.bits(3)? as usize;
                            i += rep;
                        }
                        18 => {
                            let rep = 11 + br.bits(7)? as usize;
                            i += rep;
                        }
                        _ => return Err("deflate: bad code-length symbol".into()),
                    }
                }
                if i > lengths.len() {
                    return Err("deflate: code length overflow".into());
                }
                let lit = Huffman::new(&lengths[..hlit]);
                let dist = Huffman::new(&lengths[hlit..]);
                inflate_block(&mut br, out, &lit, &dist)?;
            }
            _ => return Err("deflate: invalid block type".into()),
        }
        if bfinal == 1 {
            break;
        }
    }
    Ok(())
}

fn inflate_block(
    br: &mut BitReader,
    out: &mut Vec<u8>,
    lit: &Huffman,
    dist: &Huffman,
) -> Result<(), String> {
    loop {
        let sym = lit.decode(br)?;
        if sym < 256 {
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let idx = sym as usize - 257;
            if idx >= 29 {
                return Err("deflate: bad length symbol".into());
            }
            let len = LEN_BASE[idx] as usize + br.bits(LEN_EXTRA[idx] as u32)? as usize;
            let dsym = dist.decode(br)? as usize;
            if dsym >= 30 {
                return Err("deflate: bad distance symbol".into());
            }
            let d = DIST_BASE[dsym] as usize + br.bits(DIST_EXTRA[dsym] as u32)? as usize;
            if d > out.len() {
                return Err("deflate: distance too far back".into());
            }
            let start = out.len() - d;
            for k in 0..len {
                let b = out[start + k];
                out.push(b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_block() {
        // one final stored block containing "dexn"
        let data = [0x01, 0x04, 0x00, 0xFB, 0xFF, b'd', b'e', b'c', b'x'];
        let mut out = Vec::new();
        inflate(&data, &mut out).unwrap();
        assert_eq!(out, b"decx");
    }

    #[test]
    fn fixed_block_hello() {
        // DEFLATE fixed-block encoding of "hello" (produced by zlib level 9, commonly cited)
        let data: [u8; 10] = [
            0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00, 0x00, 0x00, 0xff, // note: trailing adler-ish; inflate stops at EOB
        ];
        let mut out = Vec::new();
        let _ = inflate(&data, &mut out);
        assert_eq!(out, b"hello");
    }
}
