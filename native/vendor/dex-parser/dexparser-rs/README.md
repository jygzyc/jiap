# dex-parser (Rust)

Pure Rust DEX file format parser.

## How it works

### DEX file layout

A DEX file is a contiguous byte buffer. The header gives offsets and sizes for every section; the parser reads index tables first, then resolves strings/types/methods on demand from the data section.

```
  +------------------+
  |  header_item    |  0x00  magic "dex\n", version, file_size, offsets...
  +------------------+
  |  string_ids[]   |  N × 4 bytes: offset → string_data_item (in data)
  |  type_ids[]     |  N × 4 bytes: descriptor_idx → string_ids
  |  proto_ids[]    |  N × 12: shorty_idx, return_type_idx, parameters_off
  |  field_ids[]    |  N × 8: class_idx, type_idx, name_idx
  |  method_ids[]   |  N × 8: class_idx, proto_idx, name_idx
  |  class_defs[]   |  N × 32: class_idx, access_flags, superclass_idx,
  |                 |         class_data_off, static_values_off, ...
  +------------------+
  |  map_list       |  lists (type, count, offset) for every section
  +------------------+
  |  data section   |  string_data (MUTF-8), type_list, class_data_item,
  |                 |  code_item (registers, insns), ...
  +------------------+
```

### Index resolution

Names and types are not stored inline; they are referenced by index. Resolving a method or field means following these chains:

```
  get_method_info(method_idx)
       │
       ▼
  method_ids[method_idx]  ──►  class_idx ──►  type_ids[class_idx]  ──►  string_ids[...]  ──►  "Lcom/foo/Bar;"
       │                           │
       ├──►  name_idx  ───────────┼────────►  string_ids[name_idx]  ──►  "doSomething"
       │                           │
       └──►  proto_idx  ──►  proto_ids[proto_idx]  ──►  return_type_idx, parameters_off
                                    │
                                    └──►  type_ids[...]  ──►  "V", "I", "Ljava/lang/String;", ...
```

```
  get_string(idx)  ──►  string_ids[idx] (offset)  ──►  data[offset]: uleb128 utf16_size + MUTF-8 bytes + 0
  get_type(idx)    ──►  type_ids[idx] (descriptor_idx)  ──►  get_string(descriptor_idx)
```

### Parsing flow

```
  &[u8] (file bytes)
       │
       ▼  DexFile::parse()
  +─────────────+
  |   DexFile    |  header + strings/types/protos/fields/methods (index tables only)
  +─────────────+
       │
       │  DexHelper::from_dex(&dex)
       ▼
  +─────────────+
  |  DexHelper  |  high-level view over the same DexFile
  +─────────────+
       │
       ├──►  classes()   ──►  for each class_def: class_idx/superclass_idx → get_type() → ClassInfo
       │
       ├──►  methods()   ──►  for each class_data: direct_methods + virtual_methods
       │                           │
       │                           ├──►  method_idx_diff (uleb128) → method_idx
       │                           ├──►  get_method_info(method_idx) → MethodInfo
       │                           └──►  code_off → get_code_item() → CodeItem (optional)
       │
       └──►  fields()    ──►  for each class_data: static_fields + instance_fields
                                    │
                                    └──►  field_idx_diff → get_field_info(field_idx) → FieldInfo
```

### Class → class_data → code

Classes are defined in `class_defs[]`. Each entry can point to a `class_data_item` (offset in the data section). Class data holds sizes and encoded field/method lists; methods can point to a `code_item` for bytecode.

```
  class_defs[i]
       │
       ├──  class_idx  ──►  type name (e.g. "Lcom/example/Main;")
       ├──  superclass_idx  ──►  super type (or NO_INDEX)
       └──  class_data_off  ──►  (if ≠ 0)
                  │
                  ▼
            class_data_item
                  │
                  ├──  static_fields_size, instance_fields_size
                  ├──  direct_methods_size, virtual_methods_size
                  │
                  ├──  encoded_field[]  (field_idx_diff, access_flags)  ──►  field_ids
                  └──  encoded_method[] (method_idx_diff, access_flags, code_off)
                                                       │
                                                       ▼  (if code_off ≠ 0)
                                                 code_item
                                                       │
                                                       ├──  registers_size, ins_size, outs_size
                                                       ├──  tries_size, debug_info_off
                                                       └──  insns_size, insns[] (Dalvik bytecode)
```

## CLI

Run the command-line tool to test the parser on a DEX file:

```bash
cargo run --release --bin dexparser -- -i path/to/classes.dex
```

Options:

- `-i, --input <FILE>` – Input DEX file (required)
- `-s, --strings` – Extract and print all strings
- `-v, --verbose` – Verbose (method params, code_off, code_item details)

Example:

```bash
dexparser -i classes.dex -s -v
```

### Batch parse with timing: `dexparse-dir`

Parse all DEX files in a directory and print time per file:

```bash
cargo run --release --bin dexparse-dir -- -d /path/to/dir           # by default: detect by magic (any extension)
cargo run --release --bin dexparse-dir -- -d /path/to/dir -r        # recursive
cargo run --release --bin dexparse-dir -- -d /path/to/dir --by-extension   # only .dex extension
```

- **Default:** read the first 4 bytes of every file; if they equal DEX magic (`dex\n`), treat the file as DEX. Finds `classes.dex`, `base.dex`, or renamed files.
- `--by-extension`: only consider files with `.dex` extension.

**Disassembly (parse + disasm timing):** run with the `disasm` feature (depends on [dex-bytecode](../dex-bytecode)). Each file is parsed and all method bytecode is disassembled; parse time and disasm time are reported separately, plus total instruction count:

```bash
cargo run --release --bin dexparse-dir --features disasm -- -d /path/to/dir
# Example: "31.41 ms parse  1487.62 ms disasm  classes.dex  (classes=5920 methods=56536 strings=66640 insns=646166)"
```

### Detecting DEX by content

To check if a buffer is DEX without parsing (e.g. when the file has no `.dex` extension):

```rust
use dex_parser::is_dex;

let bytes = std::fs::read("some_file")?;
if is_dex(&bytes) {
    let dex = dex_parser::DexFile::parse(&bytes)?;
    // ...
}
```


## Example

```rust
use dex_parser::{DexFile, DexHelper};

let bytes = std::fs::read("classes.dex")?;
let dex = DexFile::parse(&bytes)?;
let helper = DexHelper::from_dex(&dex);

// Classes
for class in helper.classes() {
    let c = class?;
    println!("CLASS {} extends {}", c.name, c.superclass_name);
}

// Methods (direct + virtual)
for method in helper.methods() {
    let m = method?;
    println!("METHOD {} {}", m.info.class, m.info.name);
    if let Some(code) = &m.code_item {
        println!("  insns_size={}", code.insns_size);
    }
}

// Fields (static + instance)
for field in helper.fields() {
    let f = field?;
    println!("FIELD {} {} {}", f.info.class, f.info.name, f.info.typ);
}

// Low-level API
let class_def = dex.get_class_def(0)?;
let class_data = dex.get_class_data(&class_def)?;
let string = dex.get_string(0)?;
let typ = dex.get_type(0)?;
```

## Layout

Crate modules map to DEX sections as follows:

```
  dex-parser
  ├── DexFile (parse, get_string, get_type, get_class_def, get_class_data, get_code_item, …)
  ├── DexHelper (classes(), methods(), fields(), strings())
  │
  └── dex/
      ├── header     →  header_item (magic, sizes, offsets)
      ├── strings    →  string_ids[] → string_data_item (MUTF-8)
      ├── types      →  type_ids[] (descriptor_idx → string)
      ├── protos     →  proto_ids[] (shorty, return_type, parameters_off)
      ├── fields     →  field_ids[] (class, type, name)
      ├── methods    →  method_ids[] (class, proto, name)
      ├── class_def  →  class_def_item (class_idx, superclass_idx, class_data_off, …)
      ├── class_data →  class_data_item (static/instance fields, direct/virtual methods)
      └── code_item  →  code_item (registers, insns, tries, …)
```

Mirrors `dex-decompiler/src/dex`. `DexHelper` provides high-level iterators over classes, methods, and fields (similar to the Python `DEXHelper`).
