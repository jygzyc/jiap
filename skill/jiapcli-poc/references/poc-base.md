---
name: poc-base
description: Exploit 基类模板，所有攻击向量的公共父类
---

# Exploit 基类

所有攻击向量实现的公共父类，提供 Activity 和日志能力。

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

## 使用方式

每个具体 Exploit 继承此基类，实现 `execute()` 方法编写攻击逻辑。`context` 已由基类持有，可直接用于启动 Activity、发送广播、查询 ContentProvider 等操作。

在 `ExploitRegistry.java` 的 `EXPLOITS` 数组中注册即可自动生成触发按钮，无需修改 UI 代码。

## 隐藏 API 调用

模板已集成 [AndroidHiddenApiBypass](https://github.com/LSPosed/AndroidHiddenApiBypass) 库，直接使用：

```java
import org.lsposed.hiddenapibypass.HiddenApiBypass;

// 调用隐藏方法
HiddenApiBypass.invoke(Class.forName("android.app.ActivityManager"), am, "someHiddenMethod", args);

// 获取隐藏方法
Method method = HiddenApiBypass.getDeclaredMethod(TargetClass.class, "methodName", paramTypes);
method.invoke(instance, args);

// 添加豁免
HiddenApiBypass.addHiddenApiExemptions("Lcom/target;");
```
