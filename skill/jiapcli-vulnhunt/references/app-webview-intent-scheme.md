# Intent Scheme 注入

WebView 中的页面通过 `intent://` scheme 触发应用内组件跳转，`shouldOverrideUrlLoading` 未拦截非白名单 scheme。**Risk: MEDIUM**


## 利用前提

需要前置条件。WebView 加载的页面包含 `intent://` scheme 链接，且 `shouldOverrideUrlLoading` 未拦截。攻击者需要能控制页面内容（XSS、HTTP 注入、或直接构造恶意链接）。点击后可触发应用内任意 Intent。

**Android 版本范围：所有版本可利用** — intent:// scheme 处理是应用层逻辑。


## 攻击流程

```
1. jiap code subclass android.webkit.WebView → 定位 WebView Activity
2. jiap code class-source <WebViewActivity> → 定位 shouldOverrideUrlLoading
3. 检查是否处理 intent:// scheme
4. 确认 Intent.parseUri() 后是否直接 startActivity()
5. 构造 intent://#Intent;component=<pkg>/<non-exported-activity>;end
6. 在 WebView 页面中注入链接触发恶意 Intent
```


## 关键特征与代码

- `shouldOverrideUrlLoading` 处理 `intent://` scheme 后直接调用 `startActivity()`
- 未校验目标组件是否在白名单内，任意组件可被跳转
- 未清除 Intent 中的敏感 Flags（如 `FLAG_GRANT_READ_URI_PERMISSION`）
- **远程攻击桥梁**：此攻击可从远程（网页）触发，是连接远程攻击面和本地提权漏洞的桥梁

```java
// 漏洞：未校验 intent:// scheme 的目标组件
webView.setWebViewClient(new WebViewClient() {
    @Override
    public boolean shouldOverrideUrlLoading(WebView view, String url) {
        if (url.startsWith("intent://")) {
            Intent intent = Intent.parseUri(url, Intent.URI_INTENT_SCHEME);
            intent.addFlags(0x10000000); // FLAG_ACTIVITY_NEW_TASK
            startActivity(intent); // 任意组件跳转
            return true;
        }
        return false;
    }
});
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2019-2032** | Chrome 的 intent:// 处理允许绕过组件导出限制访问非导出 Activity |
| **CVE-2017-5111** | Chrome shouldOverrideUrlLoading 未过滤 intent:// scheme，可触发任意组件 |
| **CVE-2020-0096 (StrandHogg 2.0)** | 利用 Activity 配置漏洞结合 intent:// scheme 实现任务劫持和钓鱼攻击 |
| **某金融 APP** | WebView 处理 intent:// 链接导致可跳转到版权管理页面重置密码 |


## 安全写法

```java
// 方案 1：拒绝 intent:// scheme（推荐）
webView.setWebViewClient(new WebViewClient() {
    private static final Set<String> ALLOWED_SCHEMES = Set.of("http", "https", "tel");

    @Override
    public boolean shouldOverrideUrlLoading(WebView view, String url) {
        Uri uri = Uri.parse(url);
        String scheme = uri.getScheme();
        // 拒绝非白名单 scheme
        if (!ALLOWED_SCHEMES.contains(scheme)) {
            Log.w(TAG, "Blocked scheme: " + scheme);
            return true;
        }
        return false;
    }
});

// 方案 2：如果必须处理 intent://，清除显式组件后校验
Intent intent = Intent.parseUri(url, Intent.URI_INTENT_SCHEME);
intent.addCategory(Intent.CATEGORY_BROWSABLE);
intent.setComponent(null);  // 清除显式组件
intent.setSelector(null);   // 清除选择器
// 清除敏感 Flags
intent.setFlags(intent.getFlags()
    & ~Intent.FLAG_GRANT_READ_URI_PERMISSION
    & ~Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
if (intent.resolveActivity(getPackageManager()) != null) {
    startActivity(intent);
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Activity Intent 重定向 | intent:// scheme 解析后跳转到非导出特权 Activity | → [[app-activity-intent-redirect]] |
| + URL 加载绕过 | 恶意页面中嵌入 intent:// 链接触发组件跳转 | → [[app-webview-url-bypass]] |
| + JS Bridge 暴露 | 通过 intent:// 跳转到 WebView Activity，再通过 JS Bridge 窃取数据 | → [[app-webview-js-bridge]] |
| + PendingIntent 滥用 | intent:// 解析的 Intent 携带 URI 权限标志 | → [[app-activity-pendingintent-abuse]] |


## Related

- [[app-activity]]
- [[app-activity-intent-redirect]]
- [[app-webview]]
- [[app-webview-url-bypass]]
