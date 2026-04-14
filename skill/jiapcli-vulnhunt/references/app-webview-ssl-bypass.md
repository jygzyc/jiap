# SSL/TLS 校验绕过

应用自定义 `WebViewClient` 时，覆盖 `onReceivedSslError` 方法并直接调用 `handler.proceed()`，忽略 SSL 错误。

**Risk: MEDIUM**

需要同一网络中间人攻击条件。解密 HTTPS 流量窃取传输中的凭证和会话信息。


## 利用前提

应用自定义 `WebViewClient.onReceivedSslError` 并调用 `handler.proceed()`。攻击者在同一网络发起 MITM 伪造证书即可截获 HTTPS 流量。

**Android 版本范围：所有版本（自定义 WebViewClient）** — 默认 WebViewClient 已自动拒绝 SSL 错误，仅当应用显式覆写 `onReceivedSslError` 调用 `proceed()` 时可利用。


## 攻击流程

```
1. jiap code subclass android.webkit.WebView → 定位 WebView Activity
2. jiap code class-source <WebViewClient> → 获取 WebViewClient 子类源码
3. jiap code xref-method "package.Class.proceed(...):void" → xref 追踪 SslErrorHandler.proceed 调用
4. 从 class-source 中定位 onReceivedSslError 方法，检查是否直接调用 handler.proceed()
5. jiap code subclass android.webkit.WebViewClient → 定位自定义 WebViewClient
6. jiap code class-source <WebViewClient> → 检查 onReceivedSslError 实现
7. 确认是否无条件调用 handler.proceed()
8. 在 MITM 环境中伪造证书，截获 HTTPS 流量
9. 提取传输中的凭证和会话信息
```


## 关键特征与代码

- `onReceivedSslError()` 中直接调用 `handler.proceed()`，无条件接受所有证书错误，导致中间人攻击

```java
// 漏洞：忽略所有 SSL 错误
@Override
public void onReceivedSslError(WebView view, SslErrorHandler handler, SslError error) {
    handler.proceed(); // 无条件信任
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2015-3837** | Android WebView onReceivedSslError 默认 proceed，导致全局 MITM 攻击 |
| **某银行 APP** | 自定义 WebViewClient 无条件 proceed，公共 WiFi 下截获用户登录信息 |
| **Google Play 审查拒绝** | 大量 APP 因 onReceivedSslError 调用 proceed() 被 Google Play 拒绝上架 |


## 安全写法

```java
// 自定义 WebViewClient，拒绝错误证书
webView.setWebViewClient(new WebViewClient() {
    @Override
    public void onReceivedSslError(WebView view, SslErrorHandler handler, SslError error) {
        // 默认拒绝所有 SSL 错误
        handler.cancel();
        // 如需提示用户，可弹出对话框让用户决定
        // showSslWarningDialog(handler, error);
    }
});

// 不要使用 TrustManager 绕过
// 以下为反模式，切勿使用：
// TrustManager[] trustAllCerts = new TrustManager[] { new X509TrustManager() { ... } };
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + JS Bridge | 中间人注入恶意 JS，通过 Bridge 调用暴露的 Java 方法 | → [[app-webview-js-bridge]] |
| + 文件访问 | SSL 绕过注入 JS 读取本地文件 | → [[app-webview-file-access]] |
| + URL 加载绕过 | MITM 结合 URL 绕过强制加载恶意页面 | → [[app-webview-url-bypass]] |


## Related

- [[app-webview]]
- [[app-webview-js-bridge]]
