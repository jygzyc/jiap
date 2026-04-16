# 模块安全分析报告

## 基本信息

| 字段 | 值 |
|------|-----|
| 目标应用 | `com.target.app` |
| 目标版本 | `x.x.x` |
| Android 版本 | `xx (API xx)` |
| 分析日期 | `YYYY-MM-DD` |

---

## 问题一：[Risk] 漏洞标题

### 1. 漏洞分析

#### 背景

> 简述问题、静态支持的利用路径，以及失效的安全边界。

#### 完整调用链

> 给出从入口到漏洞点的完整函数签名调用链。

```text
com.target.EntryActivity.onCreate(android.os.Bundle):void  （入口）
  -> getIntent().getParcelableExtra("forward_intent")
  -> startActivity(nestedIntent)
    -> com.target.InternalActivity.onCreate(android.os.Bundle):void
      -> handleIntent(intent)
        -> vulnerableOperation(data)
```

#### 代码分析

> 使用编号证据点，并直接对应结论。

1. **组件可被外部触达，且缺少有效保护**

```xml
<service
    android:name="com.target.VulnService"
    android:exported="true">
    <intent-filter>
        <action android:name="com.target.action"/>
    </intent-filter>
</service>
```

2. **服务端未校验调用者身份**

```java
@Override
public IBinder onBind(Intent intent) {
    if (checkSelfPermission("com.target.INTERNAL") != PERMISSION_GRANTED) return null;
    return mBinder; // 缺陷：检查的是自身权限，而不是调用者身份
}
```

3. **敏感操作缺少授权检查**

```java
private void deleteFile(ContentValues values, String filepath) {
    mHandler.sendMessage(mHandler.obtainMessage(MSG_DELETE, filepath));
}
```

#### 可绕过条件 / 不确定点

> 明确写出保护何时仍可被绕过，或说明为什么该发现只能保留为 `candidate`。

### 2. 攻击路径

#### 目标面

| 字段 | 值 |
|------|-----|
| 目标包名 | `com.target` |
| 目标类 | `com.target.VulnService` |
| Action / URI | `com.target.ACTION` / `content://com.target.provider/data` |
| IPC 接口 | `com.target.IService` (AIDL) / Messenger `msg.what=1` |

#### 利用步骤

> 仅描述第三方攻击者应用现实可执行的步骤。

1. `bindService` 绑定 `com.target.VulnService`
2. 获取 `IService` AIDL 接口
3. 调用 `deleteFile(filepath="/data/data/com.target/databases/secret.db")`
4. 验证文件已被删除

### 3. 真实影响

> 写出可见、可验证的安全后果。

### 4. 风险定级依据

> 对照 `references/risk-rating.md`，解释为什么是当前等级。

### 5. 修复建议

> 提供可执行的修复方案。

---

## 问题二：[Risk] 下一个漏洞标题

沿用相同结构。
