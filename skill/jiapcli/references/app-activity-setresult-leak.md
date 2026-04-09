# setResult 数据泄露

通过 `startActivityForResult` 启动目标 Activity，目标通过 `setResult` 返回未过滤的 Intent 数据，导致敏感信息泄露。

**Risk: HIGH**


## 利用前提

独立可利用。目标 Activity 通过 `setResult()` 返回包含敏感数据的 Intent。调用方（startActivityForResult）直接获取数据。需要调用方能触发目标 Activity 并读取返回结果。

**Android 版本范围：所有版本可利用** — 应用层逻辑漏洞，无系统层面修复。


## 攻击流程

```
1. jiap ard exported-components → 定位目标 Activity
2. jiap code class-source <Activity> → 搜索 setResult/startActivityForResult/onActivityResult
3. jiap code xref-method "package.Class.onActivityResult(...)" → 交叉引用追踪
4. jiap code xref-method "package.Class.setResult(...)" → 追踪 setResult 调用
5. 在反编译源码中搜索 addFlags(1)/addFlags(3) 判断是否授予 URI 权限：
   → 0x1 = FLAG_GRANT_READ_URI_PERMISSION
   → 0x3 = FLAG_GRANT_READ + WRITE
6. 恶意应用通过 startActivityForResult 启动目标导出 Activity
7. 目标 Activity 处理请求后通过 setResult(RESULT_OK, intent) 返回数据
8. 返回的 Intent 中包含未过滤的敏感信息（Token/URI/用户数据）
9. 如果返回的 Intent 携带 URI 权限 flag（0x1/0x3），恶意应用获得文件访问权
10. 恶意应用在 onActivityResult 中接收敏感数据
```


## 关键特征

- Activity 使用 `setResult(RESULT_OK, intent)` 返回数据
- 返回的 Intent 包含敏感信息（身份证号、联系人、文件 URI）
- 未校验调用者身份（`getCallingPackage()`）
- 在返回的 Intent 中携带了 URI 权限 flag（`addFlags(1)` = READ，`addFlags(3)` = READ+WRITE）


## 代码模式

```java
// 漏洞：直接返回外部传入的 Intent，可能携带 URI 权限 flag
@Override
protected void onActivityResult(int requestCode, int resultCode, Intent data) {
    // 未校验调用者，直接返回
    setResult(RESULT_OK, data);
    finish();
}

// CVE-2021-41256 变体：攻击者注入 URI 权限 flag
Intent malicious = new Intent();
malicious.setFlags(3);  // 0x3 = FLAG_GRANT_READ | FLAG_GRANT_WRITE
malicious.setData(Uri.parse("content://com.app.fileprovider/root"));
// startActivityForResult 后受害者 Activity 通过 setResult 返回此 Intent
// 攻击者获得 FileProvider 的文件读写权限
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2021-41256** | NextCloud News 通过 `startActivityForResult` 启动 SettingsActivity，在返回 Intent 中注入 `FLAG_GRANT_READ/WRITE_URI_PERMISSION`，获得 FileProvider 文件读写权限 |
| **CVE-2022-25670** | 某文件管理器导出 Activity 通过 setResult 返回 content:// URI 并携带 URI 权限，恶意应用借此读取任意私有文件 |


## 安全写法

```java
// 校验调用者 + 构建新 Intent 只返回必要数据
@Override
protected void onActivityResult(int requestCode, int resultCode, Intent data) {
    if (resultCode != RESULT_OK || data == null) {
        setResult(RESULT_CANCELED);
        finish();
        return;
    }
    String caller = getCallingPackage();
    if (!isTrustedPackage(caller)) {
        setResult(RESULT_CANCELED);
        finish();
        return;
    }
    Intent safeResult = new Intent();
    safeResult.putExtra("result_key", data.getStringExtra("public_data"));
    safeResult.setFlags(0); // 清除所有 flag，不携带 URI 权限
    setResult(RESULT_OK, safeResult);
    finish();
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Provider 数据泄露 | 通过 setResult 获得 content:// URI 授权，读取 Provider 数据 | → [[app-provider-data-leak]] |
| + URI 权限授予 | setResult 返回的 Intent 携带 FLAG_GRANT，授予恶意应用文件访问权 | → [[app-intent-uri-permission]] |
| + FileProvider 配置错误 | setResult 返回的 URI 指向配置过宽的 FileProvider，扩大文件访问范围 | → [[app-provider-fileprovider-misconfig]] |
| + Intent 重定向 | 通过 Intent 重定向触发目标 Activity，再截获 setResult 数据 | → [[app-activity-intent-redirect]] |


## Related

- [[app-activity-lifecycle]]
- [[app-activity-task-hijack]]
- [[app-provider-path-traversal]]

- [[app-activity]]
- [[app-intent]]
- [[app-intent-uri-permission]]
- [[app-provider-data-leak]]
- [[app-provider-fileprovider-misconfig]]

