# JavaScript 桥接暴露（addJavascriptInterface）

**Risk: MEDIUM → CRITICAL**（取决于暴露接口敏感度）

通过 `addJavascriptInterface` 将 Java 对象暴露给 WebView JS 层。若暴露了敏感方法（获取 Token、执行操作），攻击者通过可控页面调用即可窃取数据。


## 利用前提

①WebView 注册了含敏感 `@JavascriptInterface` 方法的桥接对象；②WebView 可加载攻击者可控页面（HTTP 注入或 intent:// 加载）。单独存在 Bridge 不构成漏洞，需配合可控页面。

**Android 版本范围：Android 10+ 可利用** — API 17+ 要求 `@JavascriptInterface` 注解，风险取决于暴露方法的敏感度。API 16 以下已有大量公开分析，此处不赘述。


## 攻击流程

```
1. jiap code subclass android.webkit.WebView → 定位 WebView 使用
2. jiap code class-source <WebViewActivity> → 获取 WebView Activity 源码
3. 从 class-source 中定位 addJavascriptInterface 调用，找到暴露的桥接对象类名
4. jiap code class-source <BridgeClass> → 获取桥接对象类源码，查看 @JavascriptInterface 方法
5. jiap code xref-method "package.Class.methodName(...):returnType" → xref 追踪暴露方法的调用链
6. 枚举 @JavascriptInterface 注解方法，评估敏感度
7. 确认 WebView 是否可加载外部 URL（结合 URL 加载绕过）
8. 利用恶意页面调用 window.<bridgeName>.<method>() 窃取数据
```


## 关键特征与代码

- 调用 `addJavascriptInterface(obj, "name")` 将对象绑定到 JS，对象方法被 `@JavascriptInterface` 注解标记，WebView 可加载外部可控 URL
- **JS 接口分两类**：① 返回数据类（获取 Token、地理位置、用户信息）；② 执行操作类（拍照、发送请求、安装应用）
- **evaluateJavascript 注入**：外部参数直接拼接到 JS 代码字符串中（`"loadPage('" + page + "')"`)，类似 DOM-based XSS

```java
// 漏洞：将敏感对象暴露给 JS
webView.getSettings().setJavaScriptEnabled(true);
webView.addJavascriptInterface(new SensitiveBridge(), "Android");

class SensitiveBridge {
    // 数据返回类
    @JavascriptInterface
    public String getToken() {
        return authToken; // JS 可读取 Token
    }

    @JavascriptInterface
    public String getUserInfo() {
        return userName + "|" + phoneNumber;
    }

    // 操作执行类
    @JavascriptInterface
    public void saveData(String data) {
        // JS 可写入数据
        prefs.edit().putString("data", data).apply();
    }

    @JavascriptInterface
    public void takePicture(String callback) {
        // JS 可触发拍照
        camera.takePicture();
    }
}
```

### 模式 2：evaluateJavascript 注入

```java
// 漏洞：外部参数直接拼接到 JS 代码
String page = getIntent().getData().getQueryParameter("page");
webView.evaluateJavascript("loadPage('" + page + "')", null);
// 或使用 loadUrl
webView.loadUrl("javascript:loadPage('" + page + "')");
// 攻击：page = '-alert(1)-'  →  loadPage('-alert(1)-')  → XSS

// 攻击页面通过 shouldOverrideUrlLoading 的 page 参数触发：
// https://legitimate.com/?page='-alert(1)-'
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **某电商 APP** | JS Bridge 暴露 getToken() 方法，通过 XSS 窃取用户 Token |
| **某社交 APP** | Bridge 暴露 getUserInfo()，配合 URL 加载绕过窃取用户资料 |


## 安全写法

```java
// 最小化暴露接口 + URL 白名单校验
webView.addJavascriptInterface(new MinimalBridge(), "bridge");

class MinimalBridge {
    @JavascriptInterface
    public String getSafeData() {
        return "non-sensitive-data"; // 仅返回非敏感数据
    }
}

// evaluateJavascript 参数必须用 JSON 转义，防止 XSS
String safeParam = org.json.JSONObject.quote(page);
webView.evaluateJavascript("loadPage(" + safeParam + ")", null);

// 移除不必要的 JS 接口
webView.removeJavascriptInterface("searchBoxJavaBridge_");
webView.removeJavascriptInterface("accessibility");
webView.removeJavascriptInterface("accessibilityTraversal");

// WebViewClient 中校验加载的 URL
webView.setWebViewClient(new WebViewClient() {
    @Override
    public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
        return !ALLOWED_ORIGINS.contains(request.getUrl().getHost());
    }
});
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + URL 加载绕过 | 远程页面加载后通过 JS Bridge 调用暴露的 Java 方法 | → [[app-webview-url-bypass]] |
| + Task 劫持 | 劫持 WebView 页面，结合 JS Bridge 窃取应用数据 | → [[app-activity-exported-access]] |
| + Deep Link 触发 | 通过 Deep Link 打开 WebView Activity，自动加载恶意 URL | → [app-intent](./app-intent.md) |
| + file:// 协议 | JS Bridge 配合 file:// 访问，读取本地文件并通过 Bridge 外发 | → [[app-webview-file-access]] |
| + SSL 绕过 | 中间人注入恶意 JS，通过 Bridge 调用暴露的 Java 方法 | → [[app-webview-ssl-bypass]] |


## Related

- [[app-activity-fragment-injection]]
- [[app-activity-task-hijack]]
- [[app-intent-uri-permission]]
- [[app-webview-intent-scheme]]

- [[app-activity-exported-access]]
- [[app-intent]]
- [[app-webview]]
- [[app-webview-url-bypass]]
