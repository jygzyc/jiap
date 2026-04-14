# getType() 信息泄露

`getType()` 方法根据 URI 返回不同 MIME 类型，攻击者可通过返回值差异探测文件是否存在，作为侦察手段。**Risk: LOW**


## 利用前提

独立可利用（但危害低）。ContentProvider 导出后，攻击者通过 `getType()` 返回值差异（null vs MIME type）探测文件是否存在。纯侦察手段，不直接造成数据泄露。

**Android 版本范围：所有版本可利用** — 但危害极低，纯侦察手段。


## 攻击流程

```
1. jiap code class-source <ProviderClass> → 从 Provider 源码检查 getType() 实现
2. jiap code xref-method "package.Provider.getType(android.net.Uri):java.lang.String" → 交叉引用追踪
3. jiap ard exported-components → 定位导出 Provider
4. jiap code class-source <ProviderClass> → 检查 getType() 实现
5. 确认 getType() 返回值是否依赖文件存在性
6. 枚举常见文件路径，通过返回值差异探测文件是否存在
7. 建立文件存在性地图，为后续攻击提供侦察信息
```


## 关键特征与代码

- `getType()` 方法根据 URI 对应的文件是否存在返回不同 MIME 类型（存在时返回具体 MIME 如 `image/jpeg`，不存在时返回 `null`），未做权限校验即可调用，返回值差异构成布尔型信息泄露

```java
// 漏洞：getType() 返回值泄露文件存在性
@Override
public String getType(Uri uri) {
    String path = uri.getPath();
    File file = new File(getContext().getFilesDir(), path);
    if (file.exists()) {
        return "image/jpeg"; // 文件存在 → 返回具体 MIME
    }
    return null; // 文件不存在 → 返回 null
    // 攻击者可通过返回值差异枚举文件路径
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2024-40676** | Android 系统 Provider 的 getType() 在不同时间返回不同 MIME，可被用于绕过 checkKeyIntent 类型检查 |
| **CVE-2023-20944** | Android 系统 Provider 的 getType/openFile 路径未规范化，通过 MIME 差异探测文件存在性 |
| **sieve.apk** | 密码管理器 Provider 的 getType() 返回值泄露文件存在性，辅助路径遍历攻击 |


## 安全写法

```java
@Override
public String getType(Uri uri) {
    // 方案 1：统一返回固定 MIME，不暴露文件存在性
    return "application/octet-stream";

    // 方案 2：需要返回 MIME 时，先校验权限
    // if (checkCallingPermission(...) != GRANTED) return null;
    // return getMimeTypeFromExtension(uri);
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + 路径遍历 | 先用 getType() 探测文件存在，再用 openFile() 遍历读取 | → [[app-provider-path-traversal]] |
| + 数据泄露 | 探测到敏感文件存在后，结合其他漏洞读取内容 | → [[app-provider-data-leak]] |
| + Intent 重定向 | 探测到的文件路径用于构造 Intent 重定向的目标 URI | → [[app-activity-intent-redirect]] |
| + CVE-2024-40676 | getType() 在不同时间返回不同 MIME，绕过 checkKeyIntent 类型检查 | → [[app-intent-parcel-mismatch]] |


## Related

- [[app-provider]]
- [[app-provider-path-traversal]]
