# ContentProvider 安全审计

ContentProvider 是 Android 数据共享的核心机制。导出的 Provider 可被任意应用查询，SQL 注入、路径遍历、数据泄露等漏洞直接影响应用数据安全。

## 风险清单

| 风险 | 等级 | 详情 |
|------|------|------|
| 数据泄露 | HIGH | [[app-provider-data-leak]] |
| SQL 注入 | HIGH | [[app-provider-sql-injection]] |
| 路径遍历 | HIGH | [[app-provider-path-traversal]] |
| call() 方法暴露 | HIGH | [[app-provider-call-expose]] |
| getType() 信息泄露 | LOW | [[app-provider-gettype-infoleak]] |
| FileProvider 配置错误 | HIGH | [[app-provider-fileprovider-misconfig]] |

## 分析流程

```
1. jiap ard exported-components -P <port>          → 列出导出 Provider
2. jiap code search-method "ContentProvider" -P <port> → 定位 Provider 实现
3. 对每个导出 Provider：
   a. jiap code class-source <Provider> -P <port>  → 获取源码
   b. 检查 query() 方法是否拼接 SQL（注入）
   c. 检查 openFile() 是否校验路径（遍历）
   d. 检查 call() 方法暴露的操作
   e. 检查 getType() 返回的信息
   f. 检查 applyBatch() / bulkInsert() 是否缺乏校验（批量操作）
4. 检查 FileProvider 配置：
   jiap code search-class "FileProvider" -P <port>
   jiap ard resource-file res/xml/file_paths.xml -P <port> → 检查路径配置
5. jiap code xref-method "android.content.ContentResolver.query(android.net.Uri,java.lang.String[],java.lang.String,java.lang.String[],java.lang.String):android.database.Cursor" -P <port> → 追踪查询调用
```

## 关键追踪模式

- **SQL 注入**：`query()` 中 `selection`/`sortOrder` 参数是否直接拼接
- **路径遍历**：`openFile()` 是否使用 `CanonicalizePath` 校验
- **FileProvider**：`file_paths.xml` 中 `root-path` / `external-path` 配置范围
- **call() 暴露**：自定义 call 方法是否执行敏感操作
- **批量操作**：`applyBatch()` / `bulkInsert()` 常缺乏单条数据的校验逻辑，且支持事务性批量操作，危害更大

## 常见误区

- **存储加密 ≠ 接口安全**：SQLCipher、Keystore 等仅保护静态文件，不保护运行时接口。Provider 暴露 = 数据明文暴露，不影响漏洞定性。

## Related

[[app-activity]]
[[app-intent]]
[[app-webview]]
[[framework-service]]
[[risk-rating]]
