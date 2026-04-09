# URL 加载绕过

**Risk: MEDIUM → HIGH**（取决于可加载的内容和攻击面）

WebView 加载攻击者可控的 URL，可能导致加载恶意页面、访问本地文件或绕过域名限制。


## 利用前提

需要前置条件。WebView 实现了 URL 白名单校验但存在绕过（如 javascript:、data:、绕过逻辑缺陷）。攻击者需要能控制 WebView 加载的 URL 或注入链接。危害取决于可绕过后加载的内容。

**Android 版本范围：所有版本可利用** — URL 白名单绕过是应用层逻辑问题，无系统修复。


## 检测模式

```bash
# Step 1: 定位 WebView Activity
jiap code subclass android.webkit.WebView

# Step 2: 获取 Activity 源码
jiap code class-source <WebViewActivity>

# Step 3: 从 class-source 中定位 loadUrl / loadData / loadDataWithBaseURL 调用
# → 检查 URL 参数来源（Intent extra / getIntent().getData()）

# Step 4: 从 class-source 中定位 shouldOverrideUrlLoading 方法
# → 检查是否做域名白名单校验
```


## 关键特征

```java
// 漏洞：从 Intent 加载任意 URL
String url = getIntent().getStringExtra("url");
webView.loadUrl(url); // 可加载 file:///data/data/com.app/shared_prefs/

// 漏洞：shouldOverrideUrlLoading 未限制（旧版 API）
@Override
public boolean shouldOverrideUrlLoading(WebView view, String url) {
    view.loadUrl(url); // 无条件加载
    return true;
}

// 漏洞：shouldOverrideUrlLoading 未限制（新版 API）
@Override
public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
    view.loadUrl(request.getUrl().toString()); // 无条件加载
    return true;
}

// 漏洞：loadDataWithBaseURL 绕过
String html = "<script>fetch('file:///data/data/com.app/shared_prefs/secret.txt')</script>";
webView.loadDataWithBaseURL("file:///data/data/com.app/files/", html, "text/html", "UTF-8", null);
```


## 攻击流程

```
1. jiap code subclass android.webkit.WebView → 定位 WebView Activity
2. jiap code class-source <WebViewActivity> → 检查 loadUrl() 参数来源
3. 检查 shouldOverrideUrlLoading 白名单逻辑
4. 尝试 javascript:, data:, file:// 等 scheme 绕过
5. 构造 adb shell am start -d "file:///..." -n <component> 验证
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2017-5111** | Chrome WebView 通过 data: URI 绕过同源策略 |
| **某社交 APP** | WebView 白名单仅检查 host，通过 javascript: scheme 绕过执行任意 JS |
| **某金融 APP** | loadUrl 参数来自 Intent extra，未检查 scheme 可加载 file:// |


## 安全写法

```java
// 使用 Uri.parse 解析 + 白名单 scheme/host 校验
private static final Set<String> ALLOWED_SCHEMES = Set.of("https");
private static final Set<String> ALLOWED_HOSTS = Set.of("app.example.com", "api.example.com");

@Override
public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
    Uri uri = request.getUrl();
    String scheme = uri.getScheme();
    String host = uri.getHost();

    // 白名单校验 scheme + host
    if (ALLOWED_SCHEMES.contains(scheme) && ALLOWED_HOSTS.contains(host)) {
        return false; // 允许加载
    }
    Log.w(TAG, "Blocked URL: " + uri);
    return true; // 拦截
}

// loadUrl 前也做校验
private void loadSafeUrl(WebView webView, String url) {
    Uri uri = Uri.parse(url);
    String scheme = uri.getScheme();
    String host = uri.getHost();
    if (ALLOWED_SCHEMES.contains(scheme) && ALLOWED_HOSTS.contains(host)) {
        webView.loadUrl(url);
    } else {
        webView.loadUrl("about:blank");
    }
}

// 安全的 loadDataWithBaseURL 校验
private void loadSafeDataWithBaseURL(WebView webView, String baseUrl, String data, String mimeType) {
    Uri baseUri = Uri.parse(baseUrl);
    // 仅允许安全的 HTTPS baseUrl
    if ("https".equals(baseUri.getScheme())) {
        webView.loadDataWithBaseURL(baseUrl, data, mimeType, "UTF-8", null);
    } else {
        webView.loadDataWithBaseURL(null, data, mimeType, "UTF-8", null);
    }
}

// 针对 loadUrl() 的白名单保护
private void loadUrlWithWhitelist(WebView webView, String url) {
    Uri uri = Uri.parse(url);
    String scheme = uri.getScheme();
    String host = uri.getHost();

    // 严格限制 scheme
    if (!ALLOWED_SCHEMES.contains(scheme)) {
        Log.w(TAG, "Blocked scheme: " + scheme);
        return;
    }

    // 严格限制 host
    if (!ALLOWED_HOSTS.contains(host)) {
        Log.w(TAG, "Blocked host: " + host);
        return;
    }

    webView.loadUrl(url);
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + JS Bridge | 加载恶意页面后通过 JS Bridge 调用暴露的方法窃取数据 | → [[app-webview-js-bridge]] |
| + file:// 协议 | 加载 file:// URI 读取应用私有文件 | → [[app-webview-file-access]] |
| + URI 权限授予 | 加载 content:// URI 访问其他应用数据 | → [[app-intent-uri-permission]] |
| + Intent Scheme | 绕过后加载的页面包含 intent:// 链接触发组件跳转 | → [[app-webview-intent-scheme]] |
| + Activity 导出越权 | 导出 WebView Activity 接受外部 URL 作为入口 | → [[app-activity-exported-access]] |


## Related

- [[app-broadcast-dynamic-abuse]]
- [[app-webview-ssl-bypass]]

- [[app-intent]]
- [[app-intent-uri-permission]]
- [[app-webview]]
- [[app-webview-file-access]]
- [[app-webview-js-bridge]]

