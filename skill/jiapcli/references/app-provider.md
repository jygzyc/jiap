# ContentProvider 安全审计总览

ContentProvider 是 Android 数据共享的核心机制。导出的 Provider 可被任意应用查询，SQL 注入、路径遍历、数据泄露等漏洞直接影响应用数据安全。

## 风险清单

| 风险 | 严重性 | 文件 |
|------|--------|------|
| 数据泄露 | HIGH | [[app-provider-data-leak]] |
| SQL 注入 | HIGH | [[app-provider-sql-injection]] |
| 路径遍历 | HIGH | [[app-provider-path-traversal]] |
| call() 方法暴露 | MEDIUM | [[app-provider-call-expose]] |
| getType() 信息泄露 | LOW | [[app-provider-gettype-infoleak]] |
| FileProvider 配置错误 | HIGH | [[app-provider-fileprovider-misconfig]] |

## 分析流程

```
1. jiap ard exported-components          → 列出导出 Provider
2. jiap code search-method "ContentProvider" → 定位 Provider 实现
3. 对每个导出 Provider：
   a. jiap code class-source <Provider>  → 获取源码
   b. 检查 query() 方法是否拼接 SQL（注入）
   c. 检查 openFile() 是否校验路径（遍历）
   d. 检查 call() 方法暴露的操作
   e. 检查 getType() 返回的信息
4. 检查 FileProvider 配置：
   jiap code search-class "FileProvider"
   jiap ard resource-file res/xml/file_paths.xml → 检查路径配置
5. jiap code xref-method "android.content.ContentResolver.query(...)" → 追踪查询调用
```

## 关键追踪模式

- **SQL 注入**：`query()` 中 `selection`/`sortOrder` 参数是否直接拼接
- **路径遍历**：`openFile()` 是否使用 `CanonicalizePath` 校验
- **FileProvider**：`file_paths.xml` 中 `root-path` / `external-path` 配置范围
- **call() 暴露**：自定义 call 方法是否执行敏感操作

## 交叉引用

- Intent URI 权限 → [[app-intent]]
- Activity 路径遍历 → [[app-activity]]
- 系统服务数据访问 → [[framework-service]]
