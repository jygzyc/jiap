# 模块安全分析报告

## 基本信息

| 字段 | 值 |
|------|-----|
| Target App | `com.target.app` |
| Target Version | `x.x.x` |
| Android Version | `xx (API xx)` |
| Analysis Date | `YYYY-MM-DD` |

---

## 问题一：[Risk] 漏洞标题

### 1. 漏洞分析

#### 漏洞背景

> 简述问题情况、利用方案和最终影响。

#### 完整调用链

> 从入口到漏洞点的完整函数签名调用链：

```
com.target.EntryActivity.onCreate(Bundle)
  → getIntent().getParcelableExtra("forward_intent")
  → startActivity(nestedIntent)
    → com.target.InternalActivity.onCreate(Bundle)
      → handleIntent(intent)
        → vulnerableOperation(data)
```

#### 漏洞代码分析

> 分条目展示漏洞代码点并说明：

1. **组件导出且未加权限保护（Manifest 层面缺失访问控制）**

```xml
<service
    android:name="com.target.VulnService"
    android:exported="true">
    <intent-filter>
        <action android:name="com.target.action"/>
    </intent-filter>
</service>
```

2. **服务端未校验调用者身份（UID / 权限 / 签名）**：

```java
@Override
public IBinder onBind(Intent intent) {
    if (checkSelfPermission("com.target.INTERNAL") != PERMISSION_GRANTED) return null;
    return mBinder; // 缺陷：检查的是自身权限而非调用者身份
}
```

3. **敏感接口未做调用者授权**：

```java
private void deleteFile(ContentValues values, String filepath) {
    mHandler.sendMessage(mHandler.obtainMessage(MSG_DELETE, filepath));
}
```

### 2. 攻击路径

#### 目标组件

| 字段 | 值 |
|------|-----|
| Target Package | `com.target` |
| Target Class | `com.target.VulnService` |
| Action / URI | `com.target.ACTION` / `content://com.target.provider/data` |
| IPC 接口 | `com.target.IService` (AIDL) / Messenger msg.what=1 |

#### 攻击步骤

> 任何第三方 app 无需特殊权限即可完成的步骤：

1. `bindService` 绑定 `com.target.VulnService`
2. 获取 `IService` AIDL 接口
3. 调用 `deleteFile(filepath="/data/data/com.target/databases/secret.db")`
4. 验证文件已被删除

### 3. 危害

> 攻击者获得什么能力、造成什么影响。

### 4. 修复方案

> 修复建议。

---

## 问题二：[Risk] 下一个漏洞标题

（同上结构）
