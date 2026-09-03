# Taint solver configuration

`dex-decompile --taint-solve` loads models from:

1. Embedded defaults (`default_config()`), or
2. `--taint-config PATH` (see [`default_taint.json`](default_taint.json)), plus optional
3. `--taint-config-extra PATH` merged on top

## Schema

```json
{
  "sources": [{"patterns": ["getIntent"], "port": "return", "kind": "ActivityUserInput"}],
  "sinks": [{"patterns": ["Runtime.exec"], "port": {"argument": {"index": 0}}, "kind": "CodeExecution"}],
  "propagations": [
    {"patterns": ["StringBuilder.append"], "from": {"argument": {"index": 1}}, "to": {"argument": {"index": 0}}}
  ],
  "sanitizers": [{"patterns": ["MessageDigest.digest"], "kinds": ["*"]}],
  "rules": [{
    "name": "RCE",
    "code": 1,
    "description": "...",
    "sources": ["ActivityUserInput"],
    "sinks": ["CodeExecution"]
  }]
}
```

Ports: `"return"`, `"this"`, or `{"argument": {"index": N}}` (0 = `this` for instance invokes).

Kinds are free-form strings; rules match source-kind → sink-kind like Mariana Trench.
