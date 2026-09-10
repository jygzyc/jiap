//! ULEB128 / SLEB128 readers (DX spec).

pub fn read_uleb128(d: &[u8], p: &mut usize) -> Result<u64, String> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        let b = *d.get(*p).ok_or("uleb128: eof")?;
        *p += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift > 63 {
            return Err("uleb128: too long".into());
        }
    }
    Ok(result)
}

pub fn read_sleb128(d: &[u8], p: &mut usize) -> Result<i64, String> {
    let mut result: i64 = 0;
    let mut shift = 0u32;
    loop {
        let b = *d.get(*p).ok_or("sleb128: eof")?;
        *p += 1;
        result |= ((b & 0x7f) as i64) << shift;
        shift += 7;
        if b & 0x80 == 0 {
            if shift < 64 && (b & 0x40) != 0 {
                result |= -1i64 << shift;
            }
            break;
        }
        if shift > 63 {
            return Err("sleb128: too long".into());
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    #[test]
    fn uleb_roundtrip() {
        let d = [0xe5, 0x8e, 0x26]; // 624485
        let mut p = 0;
        assert_eq!(super::read_uleb128(&d, &mut p).unwrap(), 624485);
        assert_eq!(p, 3);
    }

    #[test]
    fn sleb_neg() {
        // -1 = 0x7f
        let d = [0x7f];
        let mut p = 0;
        assert_eq!(super::read_sleb128(&d, &mut p).unwrap(), -1);
    }
}
