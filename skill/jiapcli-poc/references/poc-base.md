---
name: poc-base
description: Exploit 基类模板，所有攻击向量的公共父类
---

# Exploit 基类

所有攻击向量实现的公共父类，提供 Context 和日志能力。

```java
package com.poc.<target_app>.exploit;

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

在 PoCActivity 中注册到 `EXPLOITS` 数组即可自动生成触发按钮，无需修改 UI 代码。
