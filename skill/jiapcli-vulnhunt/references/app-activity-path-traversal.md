# 路径遍历（通过 Intent extras）

导出 Activity 接收文件路径参数，未校验路径合法性，导致访问任意文件。

**Risk: HIGH**


## 利用前提

独立可利用。Activity 必须是 `exported="true"`（或存在 intent-filter 自动导出），且从 Intent extras 中获取路径参数用于文件操作。攻击者无需其他漏洞配合，直接发送 Intent 即可。

**Android 版本范围：所有版本可利用** — 纯应用层逻辑漏洞，Android 系统无修复措施。


## 攻击流程

```
1. jiap code class-source <Activity> → 获取 Activity 源码，搜索文件操作相关方法
2. 搜索 getData/getDataString/getInputStream/openFileInput/openFileOutput
3. jiap code xref-method "package.Class.method(...)" → 定位 getCanonicalPath 等路径操作
4. jiap ard exported-components → 定位导出 Activity
5. jiap code class-source <Activity> → 定位文件路径参数获取入口
6. 检查路径校验逻辑是否存在
7. adb shell am start -n com.target/.FileViewActivity --es file_path "/data/data/..."
8. 读取或写入应用私有目录外的任意文件
```


## 关键特征

- Activity 从 Intent 的 data URI 或 extras 中获取文件路径
- 路径参数包含 `../` 遍历序列未过滤
- 使用 `getCanonicalPath()` 校验但校验逻辑可绕过
- 直接使用 `new File(path)` 或 `openFileInput(path)` 无白名单校验


## 代码模式

```java
// 漏洞：从 Intent 获取路径未校验
public class FileViewActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        String filePath = getIntent().getStringExtra("file_path");
        // 直接使用外部传入路径，可被遍历到 /data/data/com.app/databases/
        FileInputStream fis = new FileInputStream(filePath);
        // ...
    }
}

// 另一种：校验逻辑可绕过（先 canonical 后 startsWith）
public class SafeViewActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        String filePath = getIntent().getStringExtra("file_path");
        // 绕过：getCanonicalPath("app/data/../../../etc/passwd") = "/etc/passwd"
        File file = new File(getFilesDir(), filePath);
        String canonical = file.getCanonicalPath();
        if (canonical.startsWith(getFilesDir().getAbsolutePath())) {
            // 仍可能通过编码绕过（如 URL 编码的 ../）
            read(file);
        }
    }
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2021-25369** | 三星 sec_log Activity 接收文件路径参数未校验，导致任意文件读取 |
| **CVE-2023-20944** | Android 系统 Activity 文件路径未规范化，可访问应用沙箱外文件 |


## 安全写法

```java
public class SecureFileViewActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        String fileName = getIntent().getStringExtra("file_name");

        // 1. 仅取文件名，拒绝路径分隔符
        if (fileName == null || fileName.contains("/")
                || fileName.contains("\\")) {
            finish();
            return;
        }

        // 2. 使用 getCanonicalPath 规范化后校验前缀
        File file = new File(getFilesDir(), fileName);
        String canonical = file.getCanonicalPath();
        String allowedDir = getFilesDir().getCanonicalPath();
        if (!canonical.startsWith(allowedDir + File.separator)
                && !canonical.equals(allowedDir)) {
            finish();
            return;
        }

        // 3. 进一步使用白名单限制文件类型
        if (!fileName.endsWith(".pdf") && !fileName.endsWith(".txt")) {
            finish();
            return;
        }

        FileInputStream fis = new FileInputStream(file);
        // ...
    }
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Provider 路径遍历 | Activity 读取到的文件路径传递给 ContentProvider，扩大访问范围 | → [[app-provider-path-traversal]] |
| + URI 权限授予 | 遍历读取到的 content:// URI 通过 FLAG_GRANT 授予给恶意应用 | → [[app-intent-uri-permission]] |
| + Intent 重定向 | 通过 Intent 重定向触发内部文件访问 Activity，绕过导出限制 | → [[app-activity-intent-redirect]] |
| + WebView file:// | Activity 路径遍历读取的文件通过 WebView 加载展示 | → [[app-webview-file-access]] |


## Related

- [[app-service-intent-inject]]

- [[app-activity]]
- [[app-intent-uri-permission]]
- [[app-provider]]
- [[app-provider-path-traversal]]

