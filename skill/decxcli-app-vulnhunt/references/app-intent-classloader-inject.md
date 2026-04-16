# Intent - ClassLoader - Injection

A component or IPC surface deserializes attacker-controlled objects with an unsafe or uncontrolled `ClassLoader`. This lets the attacker influence which class is resolved and instantiated.

**Risk: HIGH**

## Exploit Prerequisites

The app reads attacker-controlled serialized data through APIs such as `getSerializableExtra()`, `readSerializable()`, `readValue()`, or an equivalent deserialization path, and the resolved class is not safely constrained.

**Android Version Scope:** Relevant across Android versions where exported components or reachable app IPC paths deserialize untrusted objects.

## Bypass Conditions / Uncertainties

- Reject the finding if the path uses strict typed deserialization or a safe Parcelable model with a fixed trusted class
- Reject the finding if the attacker cannot influence the deserialized class or payload shape
- Do not claim code execution unless the class-loading side effect is actually meaningful and reachable
- If the issue exists only behind an unreachable internal path, keep it as a chain element rather than a standalone finding

## Visible Impact

Visible impact must be concrete, such as:

- execution of attacker-influenced initialization or callback logic
- privileged handling of an attacker-controlled object type
- unsafe deserialization inside a sensitive service path

## Attack Flow

```text
1. Trace:
   -> Intent.getSerializableExtra(...)
   -> Parcel.readSerializable()
   -> Bundle.readSerializable()
   -> Parcel.readValue()
2. Confirm the deserialization path is attacker-reachable
3. Inspect whether the ClassLoader or target type is safely constrained
4. Confirm the resolved object reaches a meaningful execution or privilege boundary
```

## Key Code Patterns

- untyped or weakly typed deserialization from external input

```java
Serializable obj = intent.getSerializableExtra("config");
Config config = (Config) obj;
config.apply();
```

```java
Serializable obj = in.readSerializable();
```

## Secure Pattern

```java
SafeConfig config = intent.getParcelableExtra("config", SafeConfig.class);
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + AIDL exposure | attacker reaches deserialization through a Binder method | [[app-service-aidl-expose]] |
| + parcel mismatch | parser confusion helps smuggle an unsafe object past validation | [[app-intent-parcel-mismatch]] |
| + bound-service privilege abuse | unsafe object handling reaches a sensitive bound-service action | [[app-service-bind-escalation]] |

## Related

[[app-intent]]
[[app-service-aidl-expose]]
[[app-intent-parcel-mismatch]]
[[app-service-bind-escalation]]
