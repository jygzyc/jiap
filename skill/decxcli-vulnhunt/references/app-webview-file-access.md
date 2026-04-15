# 文件访问泄露

应用显式启用 `setAllowFileAccessFromFileURLs(true)` 或 `setAllowUniversalAccessFromFileURLs(true)`，攻击者通过 `file://` URI 读取私有文件。

**Risk: HIGH**

通过 JS 读取应用私有文件（SharedPreferences、数据库、Token）。无需用户交互。


## 利用前提

应用显式调用 `setAllowFileAccessFromFileURLs(true)` 或 `setAllowUniversalAccessFromFileURLs(true)`，且 WebView 可加载攻击者可控 URL。Android 11+ 这两个 API 已废弃且默认 false，仅当应用显式开启时可利用。

**Android 版本范围：所有版本（需应用显式开启）** — Android 11 (API 30) 起默认 false 且已废弃，但应用显式设为 true 时仍可利用。


## 攻击流程

```
1. decx code subclass android.webkit.WebView → 定位 WebView Activity
2. decx code class-source <WebViewActivity> → 检查 WebSettings 配置
3. 确认 setAllowFileAccess(true) 和 setAllowFileAccessFromFileURLs(true)
4. 检查 loadUrl() 参数来源（Intent data、extras）
5. 构造 file:///data/data/<pkg>/shared_prefs/config.xml
6. 通过 Deep Link 或 Intent 触发 WebView 加载恶意 URI
```


## 关键特征与代码

- 显式启用 `setAllowFileAccessFromFileURLs(true)` 或 `setAllowUniversalAccessFromFileURLs(true)`，且 WebView 可加载攻击者可控 URL

```java
// 漏洞：允许 file:// 协议访问
WebSettings settings = webView.getSettings();
settings.setJavaScriptEnabled(true);
settings.setAllowFileAccess(true);             // 允许文件访问
settings.setAllowFileAccessFromFileURLs(true); // 允许 JS 读取本地文件

// 利用：加载恶意 HTML 读取私有文件
// file:///data/data/com.app/shared_prefs/config.xml
```

- **content:// 访问**：WebView 默认允许访问 content:// URI（`setAllowContentAccess` 默认 true），可触发 Provider 的 `openFile()` 等危险方法

```java
// WebView 默认允许加载 content:// URI
// 如果应用有非导出 Provider 执行了危险操作（如 dumpData），可通过 WebView 触发
webView.loadUrl("content://com.victim.provider/debug");
// Provider 的 openFile() 返回的数据会被 WebView 加载和渲染
```

- **文件选择器劫持**：`onShowFileChooser()` 使用隐式 Intent 选择文件，攻击者可截获并返回受保护文件的 URI

```java
// 漏洞：onShowFileChooser 使用隐式 Intent 选择文件，攻击者可截获并返回受保护文件 URI
private ValueCallback<Uri[]> filePathCallback;

webView.setWebChromeClient(new WebChromeClient() {
    @Override
    public boolean onShowFileChooser(WebView webView, ValueCallback<Uri[]> filePathCallback,
            FileChooserParams fileChooserParams) {
        this.filePathCallback = filePathCallback;
        // 使用隐式 Intent → 恶意应用可拦截
        startActivityForResult(fileChooserParams.createIntent(), REQUEST_CODE);
        return true;
    }
});

@Override
protected void onActivityResult(int requestCode, int resultCode, Intent data) {
    if (resultCode == Activity.RESULT_OK) {
        // 未校验返回的 URI，直接传递给 WebView
        filePathCallback.onReceiveValue(new Uri[]{ data.getData() });
    }
}

// 攻击者应用：
// 1. 注册高优先级的文件选择 Activity
// 2. 在 onActivityResult 中返回受保护文件 URI
// Uri uri = Uri.parse("file:///data/user/0/com.victim/shared_prefs/secrets.xml");
// setResult(RESULT_OK, new Intent().setData(uri));
// finish();
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2020-6506** | Chrome WebView file:// 同源策略绕过，读取本地文件并外发 |
| **CVE-2019-5765** | Android WebView 通过 file:// URL 绕过同源策略读取应用私有数据 |
| **某社交 APP** | WebView 允许 file:// 访问 + Intent 可控 URL，读取登录 Token |


## 安全写法

```java
WebSettings settings = webView.getSettings();
settings.setAllowFileAccess(false);
settings.setAllowFileAccessFromFileURLs(false);
settings.setAllowUniversalAccessFromFileURLs(false);
settings.setAllowContentAccess(false); // 禁止 content:// 访问

// 文件选择器：校验返回的 URI 路径在允许范围内
@Override
protected void onActivityResult(int requestCode, int resultCode, Intent data) {
    if (resultCode == Activity.RESULT_OK && data.getData() != null) {
        Uri fileUri = data.getData();
        String path = fileUri.getPath();
        // 校验文件路径在允许的目录内
        if (path != null && path.startsWith(getFilesDir().getAbsolutePath())) {
            filePathCallback.onReceiveValue(new Uri[]{ fileUri });
        } else {
            filePathCallback.onReceiveValue(null);
        }
    }
}

// 禁用地理位置
settings.setGeolocationEnabled(false);

// Release 构建中关闭调试
if (BuildConfig.DEBUG) {
    WebView.setWebContentsDebuggingEnabled(true);
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + URL 加载绕过 | 通过 Intent 控制 loadUrl 加载 file:// 页面 | → [[app-webview-url-bypass]] |
| + Activity Intent 重定向 | 重定向到 WebView Activity 并加载 file:// URI | → [[app-activity-intent-redirect]] |
| + Provider 路径遍历 | file:// 配合 Provider 路径遍历扩大文件访问范围 | → [[app-provider-path-traversal]] |
| + JS Bridge 暴露 | 加载 file:// 页面后通过 JS Bridge 调用原生接口外发文件内容 | → [[app-webview-js-bridge]] |
| + 组件导出越权 | 导出的 WebView Activity 接受外部 URL 参数 | → [[app-activity-exported-access]] |
| + Content Provider | content:// URI 触发 Provider 的 openFile() 危险操作 | → [[app-provider]] |
| + 文件选择器劫持 | 通过 onShowFileChooser 隐式 Intent 截获文件选择，返回受保护文件 URI | → [[app-webview-url-bypass]] |


## Related

- [[app-activity-path-traversal]]
- [[app-webview-ssl-bypass]]

- [[app-activity-intent-redirect]]
- [[app-provider]]
- [[app-webview]]
- [[app-webview-url-bypass]]
