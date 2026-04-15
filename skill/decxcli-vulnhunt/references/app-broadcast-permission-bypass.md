# 权限绕过

静态注册的广播接收器声明了自定义权限保护，但权限级别设置为 `normal`（安装时自动授予），或权限声明不当。

**Risk: MEDIUM**


## 利用前提

需要前置条件。目标应用发送广播时指定的权限可被绕过（如使用 normal 级别权限而非 signature）。攻击者需要在 manifest 中声明同名 normal 权限即可注册接收器。使用 `signature` 或 `signatureOrSystem` 级别权限则不可绕过。

**Android 版本范围：所有版本可利用** — 权限绕过是应用配置问题（使用 normal 级别权限），系统层面无修复。使用 signature 级别权限不受影响。


## 攻击流程

```
1. decx ard app-manifest → 检查 <receiver> 声明和 <permission> 定义
2. 检查 <receiver> 的 android:permission 属性
3. 检查 <permission> 的 android:protectionLevel
4. 检查自定义权限的 protectionLevel（normal 可绕过）
5. 恶意应用在 manifest 中声明同名 normal 权限
6. 发送广播到受保护的接收器，触发敏感操作
7. 或注册接收器截获受权限保护的广播
```


## 关键特征与代码

- `<receiver>` 声明了 `android:permission`，但对应 `<permission>` 的 `protectionLevel` 为 `normal`，任意应用可声明并获取
- 接收器 exported 但未设置任何权限保护

```xml
<!-- 漏洞：自定义权限为 normal 级别，任意应用可声明并获取 -->
<permission
    android:name="com.example.PERMISSION_RECEIVE"
    android:protectionLevel="normal" />

<receiver
    android:name=".AdminReceiver"
    android:exported="true"
    android:permission="com.example.PERMISSION_RECEIVE">
    <intent-filter>
        <action android:name="com.example.ACTION_ADMIN" />
    </intent-filter>
</receiver>
```

```xml
<!-- 漏洞2：导出接收器无权限保护 -->
<receiver
    android:name=".DataReceiver"
    android:exported="true">
    <intent-filter>
        <action android:name="com.example.ACTION_DATA" />
    </intent-filter>
</receiver>
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2019-2012** | Android 蓝牙接收器权限级别不当，恶意应用可发送蓝牙配对请求 |
| **自定义 normal 权限** | 多款应用使用 protectionLevel=normal 保护广播接收器，任意应用可绕过 |


## 安全写法

```xml
<!-- 修复：使用 signature 级别权限 -->
<permission
    android:name="com.example.PERMISSION_RECEIVE"
    android:protectionLevel="signature" />

<receiver
    android:name=".AdminReceiver"
    android:exported="true"
    android:permission="com.example.PERMISSION_RECEIVE">
    <intent-filter>
        <action android:name="com.example.ACTION_ADMIN" />
    </intent-filter>
</receiver>

<!-- 或：非必要场景下直接设置为不导出 -->
<receiver
    android:name=".InternalReceiver"
    android:exported="false" />
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Activity Intent 重定向 | 通过受保护广播触发组件，接收器内部转发 Intent 到非导出 Activity | → [[app-activity-intent-redirect]] |
| + Service 命令注入 | 广播接收器收到恶意命令后传递给后台 Service 执行 | → [[app-service-intent-inject]] |
| + 动态广播滥用 | 当静态接收器使用 normal 权限保护时，动态注册的接收器同样可绕过 | → [[app-broadcast-dynamic-abuse]] |


## Related

- [[app-broadcast]]
