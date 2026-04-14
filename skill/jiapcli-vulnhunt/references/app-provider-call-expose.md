# call() 方法暴露

ContentProvider 的 `call()` 方法是 Android API 直接调用接口，不经过标准 CRUD 路径，覆写后无权限校验可被任意应用调用。**Risk: HIGH**


## 利用前提

独立可利用。ContentProvider 为 `exported="true"` 且 `call()` 方法无权限校验。任何应用可通过 `getContentResolver().call()` 调用自定义方法。危害取决于 `call()` 实现的功能。

**Android 版本范围：所有版本可利用** — 应用层配置问题。


## 攻击流程

```
1. jiap ard exported-components → 定位导出 Provider
2. jiap code class-source <ProviderClass> → 检查是否覆写 call() 方法
3. jiap code xref-method "package.Provider.call(java.lang.String,java.lang.String,android.os.Bundle):android.os.Bundle" → 交叉引用追踪 call() 方法
4. 分析 call() 方法的 switch/if 分支，识别敏感操作
5. 检查是否缺少权限校验（对比 query/insert 的校验逻辑）
6. adb shell content call --uri content://<authority> --method <method> --extra <params>
7. 触发敏感操作（文件操作、命令执行、数据返回）
```


## 关键特征与代码

- ContentProvider 覆写了 `call()` 方法，`call()` 内部的权限校验比 CRUD 方法更弱或完全缺失（系统不会自动检查 Manifest 中定义的权限）
- **高危害变体**：`call()` 可用于获取系统服务远程对象（PendingIntent）进一步伪造广播或启动任意组件；接受外部路径参数时可能加载外部 DEX 导致特权进程代码执行

```java
// 漏洞 1：call() 方法暴露了敏感操作且无权限校验
@Override
public Bundle call(String method, String arg, Bundle extras) {
    switch (method) {
        case "deleteFile":
            String path = extras.getString("path");
            new File(path).delete(); // 任意文件删除
            return Bundle.EMPTY;
    }
    return super.call(method, arg, extras);
}

// 漏洞 2：call() 加载外部 DEX，特权进程代码执行
@Override
public Bundle call(String method, String arg, Bundle extras) {
    if ("loadPlugin".equals(method)) {
        String dexPath = extras.getString("dexPath"); // 外部可控
        DexClassLoader loader = new DexClassLoader(dexPath,
                getContext().getCacheDir().getAbsolutePath(),
                null, getClassLoader());
        // 加载外部 DEX 中的类并执行 — 特权进程 RCE
    }
    return null;
}

// 漏洞 3：call() 返回系统服务远程对象（PendingIntent）
@Override
public Bundle call(String method, String arg, Bundle extras) {
    if ("getPendingIntent".equals(method)) {
        Bundle result = new Bundle();
        result.putParcelable("pendingIntent", getSystemPendingIntent());
        // 攻击者获取 PendingIntent 后可伪造广播或启动任意组件
        return result;
    }
    return null;
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2020-0443** | Android 系统 Provider 的 call() 方法未校验权限，可执行特权操作 |
| **CVE-2019-2021** | 三星设备 Provider call() 暴露了文件删除接口，无权限校验 |
| **CVE-2021-23243** | OPPO 系统应用通过 call() 方法加载外部传入的 DEX 文件，导致特权进程代码执行 |
| **Drozer 教程** | sieve.apk Provider call() 暴露了密码重置接口 |


## 安全写法

```java
@Override
public Bundle call(String method, String arg, Bundle extras) {
    // 校验调用者权限
    if (getContext().checkCallingOrSelfPermission("com.example.PROVIDER_ACCESS")
            != PackageManager.PERMISSION_GRANTED) {
        throw new SecurityException("Permission denied for call(): " + method);
    }
    // 限制可调用的 method 白名单
    if (!ALLOWED_METHODS.contains(method)) {
        Log.w(TAG, "Blocked call() method: " + method);
        return null;
    }
    switch (method) {
        case "getInfo":
            Bundle result = new Bundle();
            result.putString("info", getPublicInfo());
            return result;
    }
    return super.call(method, arg, extras);
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Parcel 反序列化 | 通过恶意 Bundle 传递给 call() 方法，绕过参数校验 | → [[app-intent-parcel-mismatch]] |
| + 数据泄露 | call() 返回敏感数据且无权限保护 | → [[app-provider-data-leak]] |
| + 路径遍历 | call() 方法接受文件路径参数时，可结合路径遍历访问任意文件 | → [[app-provider-path-traversal]] |
| + SQL 注入 | call() 内部执行 SQL 操作时参数未参数化 | → [[app-provider-sql-injection]] |
| + PendingIntent 获取 | call() 返回 PendingIntent 远程对象，攻击者可伪造广播或启动任意组件 | → [[app-intent-pendingintent-escalation]] |
| + DEX 加载 | call() 加载外部 DEX 文件，在特权进程中执行任意代码 | → [[app-provider-path-traversal]] |


## Related

- [[app-intent]]
- [[app-intent-parcel-mismatch]]
- [[app-provider]]
