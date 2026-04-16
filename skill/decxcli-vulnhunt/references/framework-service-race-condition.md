# Framework Service - Concurrency - Race Condition

A framework service performs security-relevant work on shared state without adequate synchronization. An attacker may win a timing window and cause duplicate actions or inconsistent privilege decisions.

**Risk: MEDIUM to HIGH**

## Exploit Prerequisites

The service exposes a concurrency-sensitive path such as check-then-act logic, shared mutable state, or non-atomic file/state transitions, and the attacker can trigger it repeatedly or concurrently.

**Android Version Scope:** Relevant across Android versions. This is a logic and synchronization issue rather than a version-specific primitive.

## Bypass Conditions / Uncertainties

- Reject the finding if synchronization fully covers the relevant shared state
- Reject the finding if winning the race causes no meaningful security consequence
- Keep severity moderate unless the raced outcome reaches privileged state changes, double-spend style abuse, or cross-user/security-policy failure
- Do not overstate exploitability if the race window is highly theoretical and no plausible trigger method exists

## Visible Impact

Visible impact must be concrete, such as:

- double execution of a protected action
- inconsistent privilege or ownership state
- duplicate reward, transfer, or policy application

## Attack Flow

```text
1. decx ard system-service-impl "<Interface>" -P <port>
2. decx code class-source "<ServiceImpl>" -P <port>
3. Inspect:
   -> check-then-act patterns
   -> shared mutable maps/lists/flags
   -> non-atomic file and rename flows
4. Confirm missing synchronization around a security-relevant decision
5. Confirm the raced outcome is visible and meaningful
```

## Key Code Patterns

- unsynchronized check-then-act on shared state

```java
public boolean transferCredits(String from, String to, int amount) {
    if (getBalance(from) >= amount) {
        deduct(from, amount);
        credit(to, amount);
        return true;
    }
    return false;
}
```

## Secure Pattern

```java
synchronized (mLock) {
    if (getBalanceLocked(from) >= amount) {
        deductLocked(from, amount);
        creditLocked(to, amount);
        return true;
    }
    return false;
}
```

## Chaining Opportunities

| Chain | Effect | Reference |
|------|--------|-----------|
| + identity confusion | raced identity state crosses user boundaries | [[framework-service-identity-confusion]] |
| + missing permission enforcement | raced sequencing weakens the permission gate | [[framework-service-permission-missing]] |
| + `clearCallingIdentity()` misuse | privileged windows widen under concurrent access | [[framework-service-clear-identity]] |

## Related

[[framework-service]]
[[framework-service-identity-confusion]]
[[framework-service-permission-missing]]
[[framework-service-clear-identity]]
