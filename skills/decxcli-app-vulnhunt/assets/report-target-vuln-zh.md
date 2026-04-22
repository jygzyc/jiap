# 漏洞 [N]：[Risk] 漏洞标题

## 1. 漏洞分析

### 背景

> 简述问题、当前已追到的利用路径，以及失效的安全边界。

### 完整调用链

> 给出从受害方组件入口到漏洞点的完整函数签名调用链。
> 起点必须是目标应用的导出组件入口或 Binder 暴露方法。
> 不要从 `AttackerApp.*`、`bindService`、`startActivity`、`sendBroadcast`、`ContentResolver.*` 等攻击者动作开始。
> 这些步骤只能写在“攻击路径 / 利用步骤”中。

```text
com.target.EntryActivity.onCreate(android.os.Bundle):void  （入口）
  -> getIntent().getParcelableExtra("forward_intent")
  -> startActivity(nestedIntent)
    -> com.target.InternalActivity.onCreate(android.os.Bundle):void
      -> handleIntent(intent)
        -> vulnerableOperation(data)
```

对于 bound service / AIDL 类问题，优先使用下面这种结构：

```text
com.target.VulnService.onBind(android.content.Intent):android.os.IBinder  （入口）
  -> return mBinder
    -> com.target.IService$Stub.deleteFile(java.lang.String):void
      -> com.target.FileHelper.delete(java.lang.String):void
```

### 代码分析

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

### 可绕过条件 / 不确定点

> 明确写出保护何时仍可被绕过，或说明为什么该发现只能保留为 `candidate`。

## 2. 攻击路径

### 目标面

| 字段 | 值 |
|------|-----|
| 目标包名 | `com.target` |
| 目标类 | `com.target.VulnService` |
| Action / URI | `com.target.ACTION` / `content://com.target.provider/data` |
| IPC 接口 | `com.target.IService` (AIDL) / Messenger `msg.what=1` |

### 利用步骤

> 仅描述第三方攻击者应用现实可执行的步骤。
> `AttackerApp.bindService(...)` 这类动作只能写在这里，不能写进“完整调用链”。

1. `bindService` 绑定 `com.target.VulnService`
2. 获取 `IService` AIDL 接口
3. 调用 `deleteFile(filepath="/data/data/com.target/databases/secret.db")`
4. 验证文件已被删除

## 3. 真实影响

> 写出可见、可验证的安全后果。

## 4. 风险定级依据

> 对照 `references/risk-rating.md` 说明定级理由。

## 5. 修复建议

> 提供可执行的修复方案。
