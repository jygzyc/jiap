# Provider 数据泄露

导出的 ContentProvider 的 `query()` 方法无权限保护，任何应用均可查询获取敏感数据。**Risk: HIGH**


## 利用前提

独立可利用。ContentProvider 为 `exported="true"` 且 `query()` 无权限校验。任何应用可直接调用 `getContentResolver().query()` 获取数据。危害取决于暴露的数据类型。

**Android 版本范围：所有版本可利用** — 应用层配置问题，ContentProvider exported + 无权限校验在所有版本均可利用。


## 攻击流程

```
1. decx ard exported-components → 定位导出 Provider
2. decx code class-source <ProviderClass> → 获取 Provider 源码
3. 检查 query() 方法是否包含权限校验：
   → 搜索 Binder.getCallingUid / checkCallingPermission
   → 检查返回的 Cursor 包含哪些列
4. 确认无 checkCallingPermission/enforcePermission 校验
5. adb shell content query --uri content://<authority>/<path>
6. 获取敏感数据（用户信息、凭证、配置）
```


## 关键特征与代码

- `<provider android:exported="true">` 未设置 `android:readPermission`，`query()` 未调用 `Binder.getCallingUid()` 或 `checkCallingPermission()` 校验，返回的 Cursor 包含敏感列
- **权限配置不对称**：设置了 `readPermission` 但未设置 `writePermission`，攻击者可通过 insert/update 间接读取或篡改数据（"写转读"绕过）

### 模式 1：无权限保护

```java
// 漏洞：query() 无权限保护返回敏感数据
@Override
public Cursor query(Uri uri, String[] projection, String selection,
                    String[] selectionArgs, String sortOrder) {
    SQLiteDatabase db = dbHelper.getReadableDatabase();
    // 无任何权限校验，直接查询并返回
    return db.query("users", projection, selection, selectionArgs,
                    null, null, sortOrder);
    // 返回的 Cursor 包含 password、token、email 等敏感列
}
```

### 模式 2：权限配置不对称（写转读）

```xml
<!-- 漏洞：readPermission 设置了但 writePermission 缺失 -->
<provider
    android:name=".MyProvider"
    android:authorities="com.victim.provider"
    android:exported="true"
    android:readPermission="com.victim.READ_DATA" />
    <!-- writePermission 未设置，任何应用可调用 insert/update -->
```

```java
// 漏洞：insert/update 操作返回敏感信息或可被用于探测数据
@Override
public Uri insert(Uri uri, ContentValues values) {
    long rowId = db.insert("users", null, values);
    // 插入成功返回的 Uri 中可能包含内部 ID，可用于推断数据结构
    return ContentUris.withAppendedId(uri, rowId);
}

@Override
public int update(Uri uri, ContentValues values, String selection, String[] selectionArgs) {
    // 无 writePermission 保护，攻击者可篡改数据
    // 若 selection 参数可被用于盲注，还可间接读取数据
    return db.update("users", values, selection, selectionArgs);
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **sieve.apk** | 密码管理器导出 Provider 无权限保护，直接 query 获取所有密码 |
| **CVE-2018-9493** | Android 下载管理器 Provider 无权限保护，泄露下载历史 |
| **CVE-2020-0015** | Android 服务中的 Provider 返回数据未过滤，包含敏感配置信息 |
| **CVE-2021-0321** | 通过微小的行为差异（异常信息不同）判断应用是否安装，造成信息泄露 |


## 安全写法

```java
@Override
public Cursor query(Uri uri, String[] projection, String selection,
                    String[] selectionArgs, String sortOrder) {
    // 校验调用者身份
    int callingUid = Binder.getCallingUid();
    if (callingUid != Process.myUid()) {
        if (getContext().checkCallingOrSelfPermission("com.example.READ_DATA")
                != PackageManager.PERMISSION_GRANTED) {
            throw new SecurityException("Unauthorized access from uid=" + callingUid);
        }
    }
    // 过滤敏感列，只返回允许的列
    String[] safeProjection = filterProjection(projection, ALLOWED_COLUMNS);
    SQLiteDatabase db = dbHelper.getReadableDatabase();
    return db.query("users", safeProjection, selection, selectionArgs,
                    null, null, sortOrder);
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + PendingIntent 窃取 | 以受害者身份通过 PendingIntent 访问其 Provider | → [[app-intent-pendingintent-escalation]] |
| + SQL 注入 | 无权限保护 + SQL 拼接，直接 UNION 提取全库 | → [[app-provider-sql-injection]] |
| + 路径遍历 | openFile() 无校验 + query() 无权限，读取任意文件 | → [[app-provider-path-traversal]] |
| + Intent 重定向 | 通过重定向启动内部组件，利用内部组件查询 Provider，绕过外部权限限制 | → [[app-activity-intent-redirect]] |
| + 写转读 | readPermission 存在但 writePermission 缺失，通过 insert/update 间接读取数据 | → [[app-provider-sql-injection]] |


## Related

- [[app-activity-setresult-leak]]
- [[app-provider-call-expose]]
- [[app-provider-gettype-infoleak]]

- [[app-intent]]
- [[app-intent-pendingintent-escalation]]
- [[app-provider]]
- [[app-provider-sql-injection]]
