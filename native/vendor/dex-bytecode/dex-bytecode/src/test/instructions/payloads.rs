//! Payload pseudo-instructions: packed-switch (0x0100), sparse-switch (0x0200), fill-array-data (0x0300).

use crate::decode_one;

fn le_u16(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn payload_packed_switch_decodes() {
    // packed-switch-payload: ident=0x0100 (2), size (2), first_key (4), then size×4 bytes targets
    // Decoder: len = 4 + 4 + size*4 (header 8 bytes then targets)
    let mut bytecode = vec![0x00, 0x01]; // ident
    bytecode.extend_from_slice(&le_u16(2)); // size
    bytecode.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // first_key (4 bytes)
    bytecode.extend_from_slice(&[0x04, 0x00, 0x00, 0x00]); // target 0
    bytecode.extend_from_slice(&[0x08, 0x00, 0x00, 0x00]); // target 1
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "packed-switch-payload");
    assert_eq!(ins.length() as usize, bytecode.len());
}

#[test]
fn payload_sparse_switch_decodes() {
    // sparse-switch-payload: ident=0x0200, size=1, keys (4 bytes each), targets (4 bytes each)
    let mut bytecode = vec![0x00, 0x02];
    bytecode.extend_from_slice(&le_u16(1));
    bytecode.extend_from_slice(&[0, 0, 0, 0]); // key 0
    bytecode.extend_from_slice(&[0x04, 0x00, 0x00, 0x00]); // target
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "sparse-switch-payload");
}

#[test]
fn payload_fill_array_data_decodes() {
    // fill-array-data-payload: ident=0x0300, elem_width=2, size=2, data (4 bytes)
    let mut bytecode = vec![0x00, 0x03];
    bytecode.extend_from_slice(&le_u16(2)); // elem_width
    bytecode.extend_from_slice(&[2, 0, 0, 0]); // size (32-bit LE)
    bytecode.extend_from_slice(&[0, 0, 0, 0]); // data
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "fill-array-data-payload");
}

#[test]
fn standard_opcode_0x00_not_payload() {
    // 0x0000 is nop (F10x), not a payload (payload ident has high byte 0x01/0x02/0x03)
    let bytecode = [0x00u8, 0x00];
    let ins = decode_one(&bytecode[..], 0).unwrap();
    assert_eq!(ins.mnemonic(), "nop");
}
