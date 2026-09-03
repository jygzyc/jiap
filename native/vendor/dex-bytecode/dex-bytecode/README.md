# dex-bytecode

Dalvik instruction decoder and control-flow helpers for DEX bytecode.

## Tests

Run all tests (including tests for decoder, resolver, branch targets, basic blocks, try/catch):

```bash
cargo test -p dex-bytecode
```

## Examples

Examples demonstrate the decoder & analysis features:

- **Reference resolution** — decode with a pluggable resolver so indices show as symbols:
  ```bash
  cargo run -p dex-bytecode --example resolve_example
  ```

- **Control-flow** — branch targets, labels, basic blocks:
  ```bash
  cargo run -p dex-bytecode --example control_flow_example
  ```

- **Try/catch** — format `.catch` lines from try/catch data (e.g. from another tool):
  ```bash
  cargo run -p dex-bytecode --example try_catch_example
  ```
