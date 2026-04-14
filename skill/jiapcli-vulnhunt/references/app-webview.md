# WebView 安全审计

WebView 是 Android 应用内嵌浏览器的核心组件。JS Bridge 暴露、文件访问、URL 加载绕过等漏洞可导致数据窃取、代码执行和本地文件泄露。

## 风险清单

| 风险 | 等级 | 详情 |
|------|------|------|
| JavaScript 桥接暴露 | MEDIUM→CRITICAL | [[app-webview-js-bridge]] |
| 文件访问泄露 | HIGH | [[app-webview-file-access]] |
| URL 加载绕过 | MEDIUM | [[app-webview-url-bypass]] |
| SSL 验证绕过 | HIGH | [[app-webview-ssl-bypass]] |
| Cookie 窃取 | MEDIUM | [[app-webview-cookie-theft]] |
| Intent Scheme 注入 | HIGH | [[app-webview-intent-scheme]] |

## 分析流程

```
1. jiap code subclass "android.webkit.WebView" -P <port> → 定位 WebView 使用
2. 对每个 WebView Activity/Fragment：
   a. jiap code class-source "<WebViewActivity>" -P <port> → 获取源码
   b. 检查 addJavascriptInterface 调用 → JS Bridge 暴露
   c. 检查 setJavaScriptEnabled(true) → JS 执行
   d. 检查 loadUrl / loadData / loadDataWithBaseURL → URL 加载
   e. 检查 setAllowFileAccess / setAllowFileAccessFromFileURLs → 文件访问
   f. 检查 WebViewClient / onReceivedSslError → SSL 绕过
3. 追踪数据流：
   jiap code xref-method "android.webkit.WebView.loadUrl(java.lang.String):void" -P <port>
   jiap code xref-method "android.webkit.WebView.evaluateJavascript(java.lang.String,android.webkit.ValueCallback):void" -P <port>
4. 检查 URL 来源：
   Intent extra → Deep Link → 用户输入 → 硬编码
```

## 关键追踪模式

- **JS Bridge**：`addJavascriptInterface()` 暴露的 `@JavascriptInterface` 方法
- **文件访问**：`setAllowFileAccess(true)` + 加载 `file://` 协议
- **URL 绕过**：`shouldOverrideUrlLoading()` 的白名单校验是否可绕过
- **SSL 绕过**：`onReceivedSslError()` 中 `handler.proceed()` 无条件信任
- **Intent Scheme**：`intent://` URI 解析后启动任意组件

## Related

[[app-activity]]
[[app-intent]]
[[app-provider]]
[[risk-rating]]
