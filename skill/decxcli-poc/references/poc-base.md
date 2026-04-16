---
name: poc-base
description: Base exploit contract for all DECX PoC construction references.
---

# Base PoC Contract

All component references in `decxcli-poc` assume the same exploit contract. The exploit class should be small, self-contained, and shaped around one verified finding.

## Base Class

```java
package com.poc.targetapp;

import android.content.Context;
import android.util.Log;

public abstract class Exploit {

    protected Context context;

    public Exploit(Context context) {
        this.context = context;
    }

    public abstract void execute();

    protected void log(String msg) {
        Log.i("PoC", msg);
    }
}
```

## Construction Rules

- One exploit class proves one finding
- Keep helper code inside the same class unless a helper component must exist in the Manifest
- Replace every placeholder package, class, action, URI, extra key, and Binder method with the real target values
- Log a visible proof point instead of a theory statement
- If an exploit needs two stages, model them explicitly as `capture -> trigger`

## Recommended Class Shape

```java
public class ExampleExploit extends Exploit {
    public ExampleExploit(Context context) {
        super(context);
    }

    @Override
    public void execute() {
        // 1. build trigger
        // 2. execute trigger
        // 3. log visible success signal
    }
}
```

## Success Signal Rules

Good signals:

- actual returned rows, tokens, or files
- actual component launch path reached
- actual Binder call accepted
- actual granted URI reused

Bad signals:

- "exploit finished"
- "target may be vulnerable"
- "should escalate privileges"

## Supporting Components

Add helper components only when the exploit mode truly needs them:

- helper `Activity`: task hijack, result capture, implicit-Intent interception
- helper `BroadcastReceiver`: broadcast interception or leak capture
- helper `Service`: overlay, long-lived listener, notification capture
- helper HTML asset: WebView-driven exploit that does not need a real remote server

If no helper component is required, do not add one.

## Hidden API Note

Use `AndroidHiddenApiBypass` only for framework-service or hidden-API cases.

```java
import org.lsposed.hiddenapibypass.HiddenApiBypass;

HiddenApiBypass.addHiddenApiExemptions("");
```

Do not make hidden-API setup part of ordinary app-component PoCs.
