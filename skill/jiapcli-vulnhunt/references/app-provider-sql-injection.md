# SQL 注入

ContentProvider 的 `query()` 方法接收外部 `selection` 和 `selectionArgs` 参数，如果直接拼接 SQL 语句，可导致 SQL 注入。

**Risk: HIGH**


## 利用前提

独立可利用。ContentProvider 为 `exported="true"` 且 `query()` 使用字符串拼接构造 SQL。攻击者通过 selection 参数注入 SQL 语句。危害取决于 Provider 暴露的数据库内容。

**Android 版本范围：所有版本可利用** — 应用层逻辑漏洞，SQL 注入在所有版本均可利用。


## 攻击流程

```
1. jiap ard exported-components → 定位导出 Provider
2. jiap code class-source <ProviderClass> → 检查 query() 中是否有 SQL 拼接
3. jiap code xref-method "package.Provider.query(android.net.Uri,java.lang.String[],java.lang.String,java.lang.String[],java.lang.String):android.database.Cursor" → 交叉引用追踪 query 调用
4. jiap code xref-method "package.Class.rawQuery(java.lang.String,java.lang.String[]):android.database.Cursor" → 追踪 rawQuery 调用
5. jiap code xref-method "package.Class.execSQL(java.lang.String):void" → 追踪 execSQL 调用
6. 确认 selection 参数直接拼接 SQL，未使用参数化查询
7. 确认 SQLiteQueryBuilder 未设置 setStrict(true)
8. 构造注入 payload: adb shell content query --uri content://com.app.provider/users --where "1=1 UNION SELECT password FROM secrets--"
9. 验证注入结果，获取敏感数据
```


## 关键特征与代码

- `query()` 方法中使用字符串拼接构造 SQL，外部传入的 `selection`、`sortOrder`、`projection` 参数直接嵌入
- `SQLiteQueryBuilder` 使用 `setTables()` 但未配置 `setStrict(true)`，攻击者可通过表名注入 `JOIN` 子句
- 使用 `rawQuery()` 或 `execSQL()` 且参数来自外部输入

```java
// 漏洞 1：selection 参数直接拼接
@Override
public Cursor query(Uri uri, String[] projection, String selection,
                    String[] selectionArgs, String sortOrder) {
    String sql = "SELECT * FROM users WHERE " + selection; // 直接拼接
    return db.rawQuery(sql, selectionArgs);
}

// 漏洞 2：直接拼接 getQueryParameter
String sql = "select * from user where name='" + uri.getQueryParameter("name") + "'";
Cursor cursor = db.rawQuery(sql, null);

// 漏洞 3：sortOrder 参数直接拼接
@Override
public Cursor query(Uri uri, String[] projection, String selection,
                    String[] selectionArgs, String sortOrder) {
    return db.query("users", projection, selection, selectionArgs,
                    null, null, sortOrder); // sortOrder 直接拼接，可注入 ORDER BY 子句
}

// 漏洞 4：SQLiteQueryBuilder.setTables() 未启用 strict 模式
SQLiteQueryBuilder builder = new SQLiteQueryBuilder();
builder.setTables("users " + uri.getQueryParameter("join")); // 表名可控，可注入 JOIN
// 未调用 builder.setStrict(true)
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **sieve.apk** | 密码管理器导出 Provider 的 query() 存在 SQL 注入，通过 UNION SELECT 提取所有密码 |
| **CVE-2018-9493** | Android 下载管理器 Provider 存在 SQL 注入，可获取下载历史和文件路径 |
| **CVE-2019-2025** | Android Media Provider 的 selection 参数未参数化，导致任意数据库查询 |


## 安全写法

```java
// 方案 1：使用参数化查询（selection）
String selection = "name = ?";
String[] selectionArgs = new String[]{ userInput };
Cursor cursor = db.query("users", null, selection, selectionArgs, null, null, null);

// 方案 2：SQLiteQueryBuilder 启用 strict 模式
SQLiteQueryBuilder builder = new SQLiteQueryBuilder();
builder.setStrict(true); // 拒绝表名中的 JOIN/子查询等可疑语法
builder.setTables("users");

// 方案 3：sortOrder 白名单校验
List<String> ALLOWED_ORDERS = Arrays.asList("name ASC", "date DESC");
if (!ALLOWED_ORDERS.contains(sortOrder)) {
    sortOrder = "name ASC"; // 不在白名单则使用默认排序
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + URI 权限授予 | SQL 注入获取的 content:// 数据通过 FLAG_GRANT 授予恶意应用 | → [[app-intent-uri-permission]] |
| + Activity 重定向 | 注入结果作为 Intent 参数传递到非导出组件 | → [[app-activity-intent-redirect]] |
| + Provider 数据泄露 | 无权限保护 + SQL 拼接，直接 UNION 提取全库 | → [[app-provider-data-leak]] |
| + call() 方法暴露 | SQL 注入获取表结构信息后，配合 call() 方法执行进一步操作 | → [[app-provider-call-expose]] |


## Related

- [[app-activity-intent-redirect]]
- [[app-intent-uri-permission]]
- [[app-provider]]
