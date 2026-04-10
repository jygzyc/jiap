# URI 权限授予滥用

通过 Intent 的 `FLAG_GRANT_READ_URI_PERMISSION` / `FLAG_GRANT_WRITE_URI_PERMISSION` 授予对 content:// URI 的临时访问权限。如果 Intent 被恶意应用截获，可利用该权限访问敏感数据。

**Risk: MEDIUM**


## 利用前提

需要前置条件。Intent 包含 content:// URI + `FLAG_GRANT_*`，且被发送到可被恶意应用截获的目标（隐式 Intent / setResult）。恶意应用截获后利用 URI 权限访问目标文件。如果使用 `setPackage()` 限制接收方则不可利用。

**Android 版本范围：Android 10+ 可利用** — Android 14 (API 34) 进一步限制后台 Activity 启动，可能影响部分 URI 权限利用链。


## 攻击流程

```
1. jiap code xref-method "android.content.Context.grantUriPermission(...)" → 定位 URI 权限授予
2. jiap code xref-method "android.content.Intent.addFlags(...)" → 追踪 addFlags 调用
3. jiap code xref-method "android.content.Intent.setFlags(...)" → 追踪 setFlags 调用
4. jiap code class-source <Class> → 获取源码，搜索 flag 值
5. 在反编译代码中搜索：
   → addFlags(1)  或 addFlags(0x1)  = FLAG_GRANT_READ_URI_PERMISSION
   → addFlags(2)  或 addFlags(0x2)  = FLAG_GRANT_WRITE_URI_PERMISSION
   → addFlags(3)  或 addFlags(0x3)  = READ + WRITE 组合
6. jiap code xref-method "android.content.Context.startActivity(...)" → 检查 Intent 的传递目标
7. jiap code xref-method "android.app.Activity.setResult(...)" → 检查 setResult 调用
8. 在反编译源码中搜索 addFlags(1) / addFlags(3) 等 URI 权限 flag 值
9. 检查 Intent 发送方式（startActivity / setResult）及是否限制接收方
10. 恶意应用注册同名接收器截获带 URI 权限的 Intent
11. 利用 URI 权限读取/写入目标应用的私有文件
```


## 关键特征

- `addFlags(1)` / `addFlags(0x1)` — `FLAG_GRANT_READ_URI_PERMISSION`
- `addFlags(2)` / `addFlags(0x2)` — `FLAG_GRANT_WRITE_URI_PERMISSION`
- `addFlags(3)` / `addFlags(0x3)` — READ + WRITE 组合
- Intent 包含 `content://` URI，且通过隐式 Intent 或 `setResult` 发送
- 未使用 `setPackage()` / `setComponent()` 限制接收方


## 代码模式

```java
// 反编译代码中的典型漏洞模式：
Intent intent = new Intent("com.app.VIEW_FILE");
intent.setData(contentUri);
intent.addFlags(1);        // ⚠️ 0x1 = FLAG_GRANT_READ_URI_PERMISSION
startActivity(intent);     // ⚠️ 隐式 Intent，恶意应用可截获

// setResult 变体：
Intent result = new Intent();
result.setData(contentUri);
result.addFlags(3);        // ⚠️ 0x3 = READ + WRITE
setResult(-1, result);     // ⚠️ RESULT_OK = -1
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2021-41256** | NextCloud News 通过 setResult 返回带 FLAG_GRANT 的 Intent，攻击者获得 FileProvider 读写权 |
| **CVE-2020-0188** | Android 系统组件注入 URI 权限后通过隐式 Intent 传递，恶意应用截获 |


## 安全写法

```java
// 显式指定接收方 + 及时撤销权限
Intent intent = new Intent("com.app.VIEW_FILE");
intent.setData(contentUri);
intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
intent.setPackage("com.trusted.receiver"); // ✅ 限制接收方
startActivity(intent);

// 使用完毕后撤销
revokeUriPermission(contentUri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Provider 路径遍历 | URI 指向的 Provider 存在路径遍历，扩大文件访问范围 | → [[app-provider-path-traversal]] |
| + Activity 重定向 | 重定向到内部 WebView Activity，通过 content:// 加载恶意页面 | → [[app-webview-js-bridge]] |
| + setResult 泄露 | setResult 返回带 FLAG_GRANT 的 URI，调用方可读取私有数据 | → [[app-activity-setresult-leak]] |


## Related

- [[app-activity-path-traversal]]
- [[app-intent-implicit-hijack]]
- [[app-provider-fileprovider-misconfig]]
- [[app-provider-sql-injection]]
- [[app-webview-url-bypass]]

- [[app-intent]]
- [[app-provider]]
- [[app-provider-path-traversal]]
- [[app-activity-setresult-leak]]

