# Decompiler fixtures APK

Real Java → DEX/APK regression fixture for the dex-decompiler. Each `*Fixtures` class
holds small methods that exercise one decompilation scenario with real Dalvik bytecode.

## Rebuild

```bash
./build.sh
```

Requires Android SDK (`ANDROID_HOME`), JDK (`javac`), platform `android-36`, and build-tools
with `d8`, `aapt2`, `zipalign`, `apksigner`.

Outputs (checked in for CI without SDK):

- `classes.dex` — used by `tests/decompiler/decompiler_fixtures.rs`
- `decompiler_fixtures.apk` — APK load smoke test

## Scenario catalog

| Class | Method | Exercises |
|-------|--------|-----------|
| `ControlFlowFixtures` | `ifElseChain` | if / else-if / else |
| | `nestedIfAnd` | nested if + `&&` |
| | `shortCircuitOr` | `\|\|` short-circuit |
| | `assignTernary` | ternary assignment |
| | `whileLoop` | while |
| | `doWhileLoop` | do-while |
| | `forLoopClassic` | classic for |
| | `breakInLoop` | break |
| | `forEachArray` / `forEachList` | enhanced for |
| `SwitchFixtures` | `packedSwitch` | packed switch |
| | `sparseSwitch` | sparse switch |
| | `switchOnString` | string switch (hashCode) |
| | `switchOnEnum` | enum switch |
| `EnumFixtures` | `Color` enum | enum emission + `$values` |
| | `allColors` | `Enum.values()` |
| `TryCatchFixtures` | `simpleTryCatch` | try/catch |
| | `tryFinally` | try/finally |
| | `multiCatch` | multi-catch |
| | `nestedTry` | nested try |
| | `tryWithResources` | TWR (Java 8 desugar) |
| `ArrayFixtures` | `newIntArray` / `newStringArray` | filled-new-array |
| | `fillArrayData` | fill-array + aput |
| | `sum2d` | nested loops + 2D arrays |
| `InvokeFixtures` | `chainedStringOps` | invoke chain + move-result |
| | `nullCheckContext` | null check after invoke |
| | `invokeResultInCondition` | condition naming (getSystemService) |
| `NamingFixtures` | `tokenValuesArray` | filled array + static sget operands |
| | `nullCheckAfterInvoke` | invoke; if-ne null check naming |
| `InnerFixtures` | `Outer.Inner` / `StaticInner` | inner classes |
| | `anonymousRunnable` | anonymous class |
| | `localClassCapture` | local class |
| `SyncFixtures` | `syncMethod` / `syncBlock` | synchronized |
| `LambdaFixtures` | `identityLambda` / `methodRef` | invokedynamic / lambdas |
| `ConstFixtures` | `wideConst` / `doubleConst` | wide/double constants |
| | `castsAndInstanceof` | check-cast / instanceof |
| `AlgorithmFixtures` | `bubbleSort` / `binarySearch` | nested loops, array indexing |
| | `fibonacciIterative` / `fibonacciRecursive` | iteration vs recursion |
| | `gcdEuclid` / `sieveOfEratosthenes` | while + boolean[] |
| | `quickSort` / `mergeSort` | divide-and-conquer recursion |
| | `bfsShortestPath` | queue BFS on adjacency matrix |
| `CryptoFixtures` | `sha256` / `hmacSha256` | MessageDigest, Mac |
| | `aesCbcEncrypt` / `aesCbcDecrypt` | Cipher + IvParameterSpec |
| | `pbkdf2Sha256` | PBE key derivation |
| | `secureRandomBytes` | SecureRandom |
| | `xorStream` | byte XOR loop |
| | `constantTimeEquals` | timing-safe compare loop |
