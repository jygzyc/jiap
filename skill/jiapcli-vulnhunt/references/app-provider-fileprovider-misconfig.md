# FileProvider 配置错误

FileProvider 是 Android 7.0（API 24）引入的 ContentProvider 子类，用于通过 `content://` URI 安全共享文件。配置不当可导致任意文件读写。

**Risk: HIGH**


## 利用前提

独立可利用。FileProvider 使用 `<root-path>` 或 `<external-path path=".">` 且 `exported="true"` 时，任何应用可直接访问。但即使 `exported="false"`，如果 `setResult()` 或 `startActivity()` 返回带 `FLAG_GRANT_*` 的 URI，配合路径遍历仍可利用。

**Android 版本范围：所有版本可利用** — Android 11+ FileProvider 在 manifest 合并时默认 exported=false，但显式设置 exported="true" 或路径配置错误（root-path）仍可利用。


## 攻击流程

```
1. jiap ard app-manifest → 定位 FileProvider 声明
2. jiap code implement FileProvider → 查找自定义 FileProvider 实现
3. jiap ard all-resources → 列出资源文件，定位 file_paths.xml
4. jiap ard resource-file res/xml/file_paths.xml → 获取路径配置内容
5. jiap code class-source <FileProviderClass> → 检查 getFileForUri() 路径校验
6. jiap code xref-method "package.Class.getUriForFile(...)" → 追踪 URI 权限使用
7. jiap code search-class "FLAG_GRANT_READ_URI_PERMISSION" → 搜索 FLAG_GRANT
8. jiap code xref-method "android.content.Intent.addFlags(...)" → 追踪 addFlags 调用
9. 检查路径配置是否包含 root-path/external-path path="." 等宽泛配置
10. 确认 exported 状态和 URI 权限授予机制
11. 构造 content:// URI 访问目标文件
```


## 关键特征与代码

| # | 类型 | 危害 | 触发条件 |
|---|------|------|----------|
| 1 | 路径配置过于宽泛 | 任意文件读写 | `file_paths.xml` 配置 `<root-path>` 或 `<external-path path=".">` |
| 2 | 自定义 FileProvider 实现缺陷 | 目录遍历 | 覆写 `getFileForUri()` 未校验路径 |
| 3 | Intent 重定向 + URI 权限泄露 | 权限授予恶意应用 | `setResult()`/`startActivity()` 返回含 `FLAG_GRANT_*` 的外部可控 Intent |
| 4 | FileProvider 被导出 | 直接访问文件 | `android:exported="true"` |
| 5 | grantUriPermission 参数可控 | 权限授予恶意应用 | `grantUriPermission()` 的 URI/包名来自外部输入 |

### 模式 1：路径配置过于宽泛

```xml
<!-- ⚠️ root-path 匹配整个文件系统 -->
<paths>
    <root-path name="root" path="" />
</paths>
<!-- 攻击 URI: content://com.app.fileprovider/root/data/data/com.victim/shared_prefs/secrets.xml -->

<!-- ⚠️ external-path 匹配全部外部存储 -->
<paths>
    <external-path name="external" path="." />
</paths>
<!-- 攻击 URI: content://com.app.fileprovider/external/DCIM/Camera/secret.jpg -->
```

### 模式 2：FileProvider 被导出

```xml
<!-- ⚠️ exported="true" — 任何应用可直接访问 -->
<provider
    android:name="androidx.core.content.FileProvider"
    android:authorities="com.app.fileprovider"
    android:exported="true"
    android:grantUriPermissions="true">
    <meta-data android:resource="@xml/file_paths" />
</provider>
```

### 模式 3：自定义 FileProvider 路径遍历

```java
// ⚠️ 直接使用 URI 路径，未校验是否在允许范围内
public class CustomFileProvider extends FileProvider {
    @Override
    public File getFileForUri(Uri uri) {
        String path = uri.getEncodedPath(); // 直接取路径，未校验范围
        File file = new File(path);
        return file.getCanonicalFile();
        // 攻击: content://authority/..%2F..%2Fdata%2Fdata%2Fcom.victim%2Fshared_prefs%2Fsecrets.xml
    }
}
```

### 模式 4：URI 权限通过 Intent 泄露

```java
// ⚠️ setResult 返回带 URI 权限的 Intent，任何调用方可读取该文件
protected void onActivityResult(int requestCode, int resultCode, Intent data) {
    if (resultCode == RESULT_OK && data != null) {
        Uri fileUri = data.getData(); // 来自其他应用的 content:// URI
        Intent result = new Intent();
        result.setData(fileUri);
        result.addFlags(1); // ⚠️ 0x1 = FLAG_GRANT_READ_URI_PERMISSION，授予调用方读取权限
        setResult(RESULT_OK, result); // ⚠️ 调用方可利用路径遍历读取任意文件
    }
}
```

### 模式 5：直接返回外部可控 Intent（setResult 复用）

```java
// ⚠️ setResult 直接复用外部传入的 Intent，可能携带 URI 权限标志
public void sendResultAndFinish(int i, String str) {
    Intent incomingIntent = getIntent(); // 外部输入
    setResult(i, incomingIntent); // ⚠️ 直接返回，可能携带 FLAG_GRANT_READ_URI_PERMISSION
    finish();
}
// 攻击者构造含恶意 URI + FLAG_GRANT 的 Intent → 受害应用返回时授予攻击者私有 Provider 访问权限
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2021-25410** | 三星 CallBGProvider 导出 + 权限 normal + openFile() 目录遍历，任意文件读取 |
| **CVE-2021-20722** | 三星 FactoryCameraFB setResult() Intent 重定向 + `<root-path>` 配置，任意文件读写 |
| **CVE-2023-20963** | DJI 遥控器 Parcel Mismatch + Settings root-path FileProvider，system 任意代码执行 |
| **CVE-2021-41256** | NextCloud News 通过 setResult 返回恶意 Intent，授予攻击者访问私有 FileProvider 的权限 |


## 安全写法

```xml
<!-- 路径配置最小化：只暴露特定子目录，禁止 root-path -->
<paths>
    <cache-path name="shared_cache" path="shared/" />
    <files-path name="exports" path="exports/" />
</paths>

<!-- FileProvider 不导出 -->
<provider
    android:name="androidx.core.content.FileProvider"
    android:authorities="com.app.fileprovider"
    android:exported="false"
    android:grantUriPermissions="true">
    <meta-data android:resource="@xml/file_paths" />
</provider>
```

```java
// Intent 过滤：不要直接转发外部 Intent
protected void onActivityResult(int requestCode, int resultCode, Intent data) {
    if (resultCode == RESULT_OK && data != null) {
        // 方案 1：创建新 Intent，不复用外部输入
        Intent result = new Intent();
        result.setData(data.getData()); // 仅传递必要的 URI
        // 不添加 FLAG_GRANT_* 标志
        setResult(RESULT_OK, result);

        // 方案 2：校验目标包名
        // if (data.getComponent() != null
        //     && data.getComponent().getPackageName().equals(getPackageName())) {
        //     setResult(RESULT_OK, data); // 仅允许内部组件
        // }
    }
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Intent 重定向 | setResult() 获取 URI 权限 + 路径遍历读写任意文件 | → [[app-activity-intent-redirect]] |
| + Parcel 读写不对称 | 利用 Parcelable 反序列化获取系统权限 + 系统 FileProvider 读写系统文件 | → [[app-intent-parcel-mismatch]] |
| + 组件导出越权 | 导出组件暴露文件访问接口 + 路径遍历 | → [[app-activity-exported-access]] |
| + PendingIntent 滥用 | PendingIntent 目标可控 + 携带 URI 权限标志 | → [[app-activity-pendingintent-abuse]] |
| + URI 权限授予 | URI 权限被截获，扩大文件访问范围 | → [[app-intent-uri-permission]] |


## Related

- [[app-activity-setresult-leak]]
- [[app-provider-path-traversal]]
- [[framework-service-intent-redirect]]

- [[app-activity]]
- [[app-provider]]
- [[app-intent-uri-permission]]
