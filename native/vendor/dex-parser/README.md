<p align="center"><img width="120" src="./.github/logo.png"></p>
<h2 align="center">DEX-Parser: The Scalpel for Dalvik Executables</h2>

<div align="center">

![Powered By: Androguard](https://img.shields.io/badge/androguard-green?style=for-the-badge&label=Powered%20by&link=https%3A%2F%2Fgithub.com%2Fandroguard)
![Sponsor](https://img.shields.io/badge/sponsor-nlnet-blue?style=for-the-badge&link=https%3A%2F%2Fnlnet.nl%2F)
![PYPY](https://img.shields.io/badge/PYPI-DEXPARSER-violet?style=for-the-badge&link=https%3A%2F%2Fpypi.org%2Fproject%2Fdexparser-ag%2F)


</div>

# Description

The soul of every Android app is its code, compiled into a compact, efficient Dalvik Executable (DEX) format. `dex-parser` is the surgical tool designed to lay this soul bare.

This is a standalone, dependency-free, native Python library built to parse the complete structure of DEX files. It is a core pillar of the new Androguard Ecosystem, providing a high-fidelity map of an application's code layout—its classes, methods, fields, and strings—before deeper analysis begins.

# Philosophy

Following the "Deconstruct to Reconstruct" philosophy, dex-parser operates as a specialized, independent library. It does not concern itself with the meaning of the bytecode; its singular focus is on perfectly and performantly reading the blueprint of the executable. This separation of concerns makes it a robust and reliable foundation for any tool that needs to understand the structure of Dalvik code.

# Key Features


- Full Structure Parsing: Reads and indexes the entire DEX file, including the header, string table, type identifiers, method prototypes, and class definitions.
- Class & Method Enumeration: Provides a clean, Pythonic API to iterate through all defined classes, their methods (both direct and virtual), and their fields.
- On demand access for each fields by using [Hachoir library](https://github.com/vstinner/hachoir).
- Cross-Reference Ready: Lays the groundwork for building cross-references by cleanly separating method and field definitions from their invocations.
- **Rust core with Python bindings:** The parser is implemented in Rust (`dexparser-rs`) and exposed to Python via PyO3 (`dexparser-py`). Fast parsing with the same high-level API (`DEX`, `DEXHelper`).
- [TODO] Multi-DEX Aware: Natively understands and can parse classes.dex, classes2.dex, and so on, providing a unified view of the application's code.

## Installation

Requires Rust (for the native extension). Use a virtual environment:

```bash
git clone https://github.com/androguard/dex-parser.git
cd dex-parser
python3 -m venv .venv && source .venv/bin/activate
pip install maturin
maturin develop --manifest-path dexparser-py/Cargo.toml
```

Or via PyPI (when published):

```bash
pip install dexparser-ag
```

## Examples

You can directly use it by command line to parse and display quickly information about a DEX file, but the purpose of this tool is mainly to be a library for other tools like Androguard.

```
$ dexparser -i Test.dex
```

## Usage

Open a DEX file with the `DEX` class (file path, bytes, or a readable stream):

```python
from dexparser import DEX, DEXHelper, DEX_from_source

d = DEX.from_path("classes.dex")
# or: d = DEX(bytes_data)
# or: d = DEX_from_source(open("classes.dex", "rb"))  # legacy stream API

print(d["header"])  # dict with file_size, class_defs_size, ...
```

Use `DEXHelper` to iterate classes, methods, and fields:

```python
dh = DEXHelper.from_rawdex(d)

for cls in dh.get_classes():
    print("CLASS", cls.name, cls.sname)

for method in dh.get_methods():
    print("METHOD", method.class_name, method.name, method.proto)
    code = method.get_code()
    if code:
        insns = code["insns"].value  # raw Dalvik bytecode bytes
        print("  CODE", code.debug_info_off, code.insns_size, len(insns))

for field in dh.get_fields():
    print("FIELD", field.class_name, field.name, field.type_field)
```

`DEXHelper.from_string(data)` parses from a `bytes` buffer directly.

## Rust implementation

The parser core lives in `dexparser-rs/`. Python bindings are in `dexparser-py/` (PyO3 module `dexparser_rs`). You can use the library from Rust without Python.

### Parsing flow

```
  &[u8] (file bytes)
       │
       ▼  DexFile::parse()
  +─────────────+
  |   DexFile    |  header + string_ids, type_ids, proto_ids, field_ids, method_ids (index tables)
  +─────────────+
       │
       │  DexHelper::from_dex(&dex)
       ▼
  +─────────────+
  |  DexHelper  |  high-level iterators over the same DexFile
  +─────────────+
       │
       ├──►  classes()   ──►  ClassInfo (name, superclass_name) per class_def
       ├──►  methods()   ──►  MethodInfoItem (class, name, proto, code_item) per direct/virtual method
       └──►  fields()    ──►  FieldInfoItem (class, name, type) per static/instance field
```

### DEX file layout (what the parser reads)

```
  +------------------+
  |  header_item    |  magic "dex\n", version, file_size, offsets for every section
  +------------------+
  |  string_ids[]   |  offset → string_data (MUTF-8) in data section
  |  type_ids[]     |  descriptor_idx → string_ids
  |  proto_ids[]    |  shorty_idx, return_type_idx, parameters_off
  |  field_ids[]    |  class_idx, type_idx, name_idx
  |  method_ids[]   |  class_idx, proto_idx, name_idx
  |  class_defs[]   |  class_idx, superclass_idx, class_data_off, ...
  +------------------+
  |  map_list       |  (type, count, offset) for each section
  +------------------+
  |  data section   |  string_data, type_list, class_data_item, code_item (insns), ...
  +------------------+
```

### CLI tools

```
  dexparser          single file: parse and print header, classes, methods, fields
       │             usage:  dexparser -i classes.dex [-s] [-v]
       │
  dexparse-dir       directory: find DEX files (by magic or .dex), parse each, report time per file
       │             usage:  dexparse-dir -d /path [-r] [--by-extension]
       │
       └──►  with --features disasm (dex-bytecode):  parse + disassemble all method bytecode
                                     │
                                     ▼
                              "X.XX ms parse  Y.YY ms disasm  file.dex  (classes=... insns=...)"
```

See `dexparser-rs/README.md` for API details, dependency, and optional disassembly with [dex-bytecode](https://github.com/androguard/dex-bytecode).

## License

Distributed under the [Apache License, Version 2.0](LICENSE).

