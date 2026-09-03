//! Helpers for building minimal DEX bytes in tests.

/// Push uleb128 encoding of `value` into `out`.
pub fn push_uleb128(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut b = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            b |= 0x80;
        }
        out.push(b);
        if value == 0 {
            break;
        }
    }
}

/// Minimal valid DEX with no classes (header + map only). Decompiles to empty string.
pub fn minimal_dex_bytes() -> Vec<u8> {
    let mut data = vec![0u8; 0x80];
    data[0..4].copy_from_slice(&[0x64, 0x65, 0x78, 0x0a]);
    data[4..8].copy_from_slice(b"035\0");
    data[32..36].copy_from_slice(&(0x80u32).to_le_bytes());
    data[36..40].copy_from_slice(&(0x70u32).to_le_bytes());
    data[40..44].copy_from_slice(&(0x1234_5678u32).to_le_bytes());
    data[52..56].copy_from_slice(&(0x70u32).to_le_bytes());
    for i in (56..112).step_by(4) {
        data[i..i + 4].copy_from_slice(&0u32.to_le_bytes());
    }
    data[0x70..0x74].copy_from_slice(&(1u32).to_le_bytes());
    data[0x74..0x78].copy_from_slice(&(0u32).to_le_bytes());
    data[0x78..0x7c].copy_from_slice(&(0x70u32).to_le_bytes());
    data[0x7c..0x80].copy_from_slice(&(0u32).to_le_bytes());
    data
}

/// Build a minimal valid DEX with one class `LSimple;`, one direct method `foo()V`, and
/// method body = `insns` (raw Dalvik instruction bytes). Used to test decompiled output patterns.
pub fn minimal_dex_with_method_code(insns: &[u8]) -> Vec<u8> {
    // String indices: 0=V, 1=Ljava/lang/Object;, 2=LSimple;, 3=foo, 4=()V
    let mut data_section = Vec::new();
    let mut string_offsets = Vec::new();
    for (utf16_len, s) in [
        (1u32, "V"),
        (18, "Ljava/lang/Object;"),
        (8, "LSimple;"),
        (3, "foo"),
        (3, "()V"),
    ] {
        string_offsets.push(data_section.len() as u32);
        push_uleb128(&mut data_section, utf16_len);
        data_section.extend_from_slice(s.as_bytes());
        data_section.push(0);
    }
    while data_section.len() % 4 != 0 {
        data_section.push(0);
    }
    let type_list_off = data_section.len();
    data_section.extend_from_slice(&0u32.to_le_bytes()); // type_list size 0
    let class_data_off = data_section.len();
    push_uleb128(&mut data_section, 0); // static_fields_size
    push_uleb128(&mut data_section, 0); // instance_fields_size
    push_uleb128(&mut data_section, 1); // direct_methods_size
    push_uleb128(&mut data_section, 0); // virtual_methods_size
    push_uleb128(&mut data_section, 0); // method_idx_diff (first method -> index 0)
    push_uleb128(&mut data_section, 1); // access_flags (public)
                                        // code_off = data_off + (current len + 2 for uleb) so code_item starts at data_off + class_data_len
    let data_off_val = 0x154u32;
    let code_item_start_in_data = (data_section.len() + 2) as u32; // +2 for uleb128(code_off)
    push_uleb128(&mut data_section, data_off_val + code_item_start_in_data);
    let code_item_start = data_section.len();
    data_section.extend_from_slice(&2u16.to_le_bytes()); // registers_size
    data_section.extend_from_slice(&0u16.to_le_bytes()); // ins_size
    data_section.extend_from_slice(&0u16.to_le_bytes()); // outs_size
    data_section.extend_from_slice(&0u16.to_le_bytes()); // tries_size
    data_section.extend_from_slice(&0u32.to_le_bytes()); // debug_info_off
    let insns_units = (insns.len() + 1) / 2;
    data_section.extend_from_slice(&(insns_units as u32).to_le_bytes()); // insns_size
    data_section.extend_from_slice(insns);
    if insns.len() % 2 != 0 {
        data_section.push(0);
    }

    let data_off = 0x154u32; // chosen so header + index sections + map fit before
    let string_ids_off = 0x70u32;
    let type_ids_off = 0x84u32;
    let proto_ids_off = 0x8cu32;
    let field_ids_off = 0x98u32;
    let method_ids_off = 0x98u32;
    let class_defs_off = 0xa0u32;
    let map_off = 0xc0u32;

    let mut out = Vec::new();
    out.extend_from_slice(&[0x64u8, 0x65, 0x78, 0x0a]);
    out.extend_from_slice(b"035\0");
    out.resize(32, 0);
    let file_size = (data_off as usize) + data_section.len();
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0x70u32.to_le_bytes());
    out.extend_from_slice(&0x1234_5678u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // link_size
    out.extend_from_slice(&0u32.to_le_bytes()); // link_off
    out.extend_from_slice(&map_off.to_le_bytes());
    out.extend_from_slice(&5u32.to_le_bytes());
    out.extend_from_slice(&string_ids_off.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&type_ids_off.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&proto_ids_off.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&field_ids_off.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&method_ids_off.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&class_defs_off.to_le_bytes());
    out.extend_from_slice(&(data_section.len() as u32).to_le_bytes());
    out.extend_from_slice(&data_off.to_le_bytes());

    out.resize(string_ids_off as usize, 0);
    for &off in &string_offsets {
        out.extend_from_slice(&(data_off + off).to_le_bytes());
    }
    out.resize(type_ids_off as usize, 0);
    out.extend_from_slice(&1u32.to_le_bytes()); // type 0 = Object (string 1)
    out.extend_from_slice(&2u32.to_le_bytes()); // type 1 = Simple (string 2)
    out.resize(proto_ids_off as usize, 0);
    out.extend_from_slice(&4u32.to_le_bytes()); // shorty_idx = 4 ()V
    out.extend_from_slice(&0u32.to_le_bytes()); // return_type_idx = 0 (V)
    out.extend_from_slice(&0u32.to_le_bytes()); // parameters_off = 0
    out.resize(method_ids_off as usize, 0);
    out.extend_from_slice(&1u16.to_le_bytes()); // class_idx = 1
    out.extend_from_slice(&0u16.to_le_bytes()); // proto_idx = 0
    out.extend_from_slice(&3u32.to_le_bytes()); // name_idx = 3 (foo)
    out.resize(class_defs_off as usize, 0);
    out.extend_from_slice(&1u32.to_le_bytes()); // class_idx = 1
    out.extend_from_slice(&0x01u32.to_le_bytes()); // access public
    out.extend_from_slice(&0u32.to_le_bytes()); // superclass_idx = 0
    out.extend_from_slice(&0u32.to_le_bytes()); // interfaces_off
    out.extend_from_slice(&0xffff_ffffu32.to_le_bytes()); // source_file_idx
    out.extend_from_slice(&0u32.to_le_bytes()); // annotations_off
    out.extend_from_slice(&(data_off + class_data_off as u32).to_le_bytes()); // class_data_off
    out.extend_from_slice(&0u32.to_le_bytes()); // static_values_off

    out.resize(map_off as usize, 0);
    out.extend_from_slice(&12u32.to_le_bytes()); // map size
    let map_entries: [(u16, u32, u32); 12] = [
        (0x0000, 1, 0),                                 // header
        (0x0002, 5, string_ids_off),                    // string_id
        (0x0003, 2, type_ids_off),                      // type_id
        (0x0004, 1, proto_ids_off),                     // proto_id
        (0x0005, 0, field_ids_off),                     // field_id
        (0x0006, 1, method_ids_off),                    // method_id
        (0x0007, 1, class_defs_off),                    // class_def
        (0x1000, 1, map_off),                           // map_list
        (0x1001, 1, data_off + type_list_off as u32),   // type_list
        (0x2002, 5, data_off),                          // string_data (first at data_off)
        (0x2000, 1, data_off + class_data_off as u32),  // class_data
        (0x2001, 1, data_off + code_item_start as u32), // code_item
    ];
    for (ty, size, offset) in map_entries {
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
    }
    out.extend_from_slice(&data_section);
    out
}

/// Minimal valid DEX with one class `Lpkg/Test;`, one method `getList()Ljava/util/List;`, so that
/// decompiled output should contain "import java.util.List;" and short name "List" in the signature.
pub fn minimal_dex_with_list_return_type() -> Vec<u8> {
    let mut data_section = Vec::new();
    let mut string_offsets = Vec::new();
    for (utf16_len, s) in [
        (1u32, "V"),
        (18, "Ljava/lang/Object;"),
        (19, "Ljava/util/List;"),
        (10, "Lpkg/Test;"),
        (7, "getList"),
        (19, "()Ljava/util/List;"),
        (1, "L"),
    ] {
        string_offsets.push(data_section.len() as u32);
        push_uleb128(&mut data_section, utf16_len);
        data_section.extend_from_slice(s.as_bytes());
        data_section.push(0);
    }
    while data_section.len() % 4 != 0 {
        data_section.push(0);
    }
    let type_list_off = data_section.len();
    data_section.extend_from_slice(&0u32.to_le_bytes());
    let class_data_off = data_section.len();
    push_uleb128(&mut data_section, 0);
    push_uleb128(&mut data_section, 0);
    push_uleb128(&mut data_section, 1);
    push_uleb128(&mut data_section, 0);
    push_uleb128(&mut data_section, 0);
    push_uleb128(&mut data_section, 1);
    let data_off_val = 0x160u32;
    let code_item_start_in_data = (data_section.len() + 2) as u32;
    push_uleb128(&mut data_section, data_off_val + code_item_start_in_data);
    let code_item_start = data_section.len();
    data_section.extend_from_slice(&2u16.to_le_bytes());
    data_section.extend_from_slice(&0u16.to_le_bytes());
    data_section.extend_from_slice(&0u16.to_le_bytes());
    data_section.extend_from_slice(&0u16.to_le_bytes());
    data_section.extend_from_slice(&0u32.to_le_bytes());
    data_section.extend_from_slice(&1u32.to_le_bytes());
    data_section.extend_from_slice(&[0x0e, 0x00]); // return-void

    let data_off = 0x160u32;
    let string_ids_off = 0x70u32;
    let type_ids_off = 0x8cu32; // 0x70 + 7*4
    let proto_ids_off = 0x98u32;
    let field_ids_off = 0xa4u32;
    let method_ids_off = 0xa4u32;
    let class_defs_off = 0xacu32;
    let map_off = 0xccu32;

    let mut out = Vec::new();
    out.extend_from_slice(&[0x64u8, 0x65, 0x78, 0x0a]);
    out.extend_from_slice(b"035\0");
    out.resize(32, 0);
    let file_size = (data_off as usize) + data_section.len();
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0x70u32.to_le_bytes());
    out.extend_from_slice(&0x1234_5678u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&map_off.to_le_bytes());
    out.extend_from_slice(&7u32.to_le_bytes());
    out.extend_from_slice(&string_ids_off.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes());
    out.extend_from_slice(&type_ids_off.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&proto_ids_off.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&field_ids_off.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&method_ids_off.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&class_defs_off.to_le_bytes());
    out.extend_from_slice(&(data_section.len() as u32).to_le_bytes());
    out.extend_from_slice(&data_off.to_le_bytes());

    out.resize(string_ids_off as usize, 0);
    for &off in &string_offsets {
        out.extend_from_slice(&(data_off + off).to_le_bytes());
    }
    out.resize(type_ids_off as usize, 0);
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes());
    out.resize(proto_ids_off as usize, 0);
    out.extend_from_slice(&6u32.to_le_bytes()); // shorty_idx = 6 ("L")
    out.extend_from_slice(&1u32.to_le_bytes()); // return_type_idx = 1 (List)
    out.extend_from_slice(&0u32.to_le_bytes());
    out.resize(method_ids_off as usize, 0);
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&4u32.to_le_bytes());
    out.resize(class_defs_off as usize, 0);
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&0x01u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(data_off + class_data_off as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    out.resize(map_off as usize, 0);
    out.extend_from_slice(&12u32.to_le_bytes());
    let map_entries: [(u16, u32, u32); 12] = [
        (0x0000, 1, 0),
        (0x0002, 7, string_ids_off),
        (0x0003, 3, type_ids_off),
        (0x0004, 1, proto_ids_off),
        (0x0005, 0, field_ids_off),
        (0x0006, 1, method_ids_off),
        (0x0007, 1, class_defs_off),
        (0x1000, 1, map_off),
        (0x1001, 1, data_off + type_list_off as u32),
        (0x2002, 7, data_off),
        (0x2000, 1, data_off + class_data_off as u32),
        (0x2001, 1, data_off + code_item_start as u32),
    ];
    for (ty, size, offset) in map_entries {
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
    }
    out.extend_from_slice(&data_section);
    out
}
