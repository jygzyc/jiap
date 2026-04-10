# 组件导出越权访问

Activity 默认不导出，但设置 `exported="true"` 或添加 `<intent-filter>`（Android 12 以下默认导出）后变为导出组件，可被直接启动绕过认证。

**Risk: MEDIUM**

## 利用前提

独立可利用。Activity 为 `exported="true"` 且未设置 `android:permission`。但**注意**：导出本身不一定是漏洞——只有当 Activity 处理敏感操作（查看数据、修改设置、管理用户）时才是。需要人工判断 Activity 的实际功能。

**Android 版本范围：所有版本可利用** — Android 12 (API 31) 强制要求包含 intent-filter 的组件必须显式声明 `android:exported` 属性，不声明则无法安装。此变更减少了意外导出的风险，但显式设置 `exported="true"` 的组件在所有版本均可利用。

## 关键特征

- 敏感 Activity（管理后台、支付确认、设置页面）被导出
- 导出 Activity 缺少权限保护（未设置 `android:permission`）
- 内部认证逻辑仅在非导出 Activity 中执行，导出 Activity 直接展示内容
- **权限提升**：导出组件内部执行了需要特定权限的敏感操作（拨打电话、发送短信、访问私有数据），恶意应用无需申请该权限即可通过启动组件间接执行

## 代码模式

### 模式 1：敏感页面直接导出

```java
// 典型漏洞：敏感列表页面直接导出
<activity
    android:name=".PWList"
    android:exported="true" />  <!-- 密码列表页面，无认证保护 -->

// adb shell am start -n com.app/.PWList  直接绕过密码验证访问
```

### 模式 2：导出组件执行敏感操作（权限提升）

```xml
<!-- 漏洞配置：导出且无权限保护 -->
<activity android:name=".SensitiveActivity" android:exported="true" />
```

```java
// 漏洞：导出的 Activity 直接执行敏感操作，未校验调用者
public class SensitiveActivity extends Activity {
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        // 未校验调用者，直接拨打电话
        // 恶意应用无需申请 CALL_PHONE 权限即可拨打电话
        Intent callIntent = new Intent(Intent.ACTION_CALL);
        callIntent.setData(Uri.parse("tel:10086"));
        startActivity(callIntent);
    }
}

// 攻击代码：
// Intent intent = new Intent();
// intent.setClassName("com.victim", "com.victim.SensitiveActivity");
// startActivity(intent);
```

## 攻击流程

```
1. jiap ard exported-components → 列出所有导出 Activity
2. 识别敏感 Activity（名称含 Admin/Setting/Debug/Payment 等）
3. adb shell am start -n com.target/.SensitiveActivity → 直接启动
4. 绕过登录/认证屏幕，直接访问敏感功能
5. 获取敏感数据或执行特权操作
```

## 经典案例

| 案例 | 攻击场景 |
|------|------|
| **sieve.apk** | 密码管理器导出 `PWList` Activity，通过 drozer 直接启动绕过密码验证查看所有密码 |
| **CVE-2019-9465** | Google Pixel 导出的设置组件未加权限保护，可从外部直接启动 |
| **CVE-2023-21036** | Google Markup 工具导出 Activity 允许直接访问截图编辑功能 |
| **CVE-2021-41256** | NextCloud News 导出的 SettingsActivity 通过 setResult 返回外部传入的 Intent，导致任意文件读写 |

## 安全写法

```xml
<!-- 自定义 signature 权限保护导出 Activity -->
<permission
    android:name="com.app.permission.ACCESS_PWLIST"
    android:protectionLevel="signature" />

<activity
    android:name=".PWList"
    android:exported="true"
    android:permission="com.app.permission.ACCESS_PWLIST" />

<!-- 如果不需要导出，直接设为 false -->
<!-- android:exported="false" -->
```

## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Intent 重定向 | 重定向 Intent 指向导出的敏感 Activity，绕过内部访问控制 | → [[app-activity-intent-redirect]] |
| + Deep Link | 通过 Deep Link URL 远程触发导出 Activity，无需安装恶意应用 | → [[app-intent]] |
| + Fragment 注入 | 导出 Activity 加载攻击者指定的 Fragment，显示恶意界面 | → [[app-activity-fragment-injection]] |
| + 点击劫持 | 导出 Activity 的敏感操作被覆盖层误导用户点击 | → [[app-activity-clickjacking]] |
| + Intent 重定向入口 | 导出组件常作为 Intent 重定向攻击的入口点，攻击者通过重定向到达非导出组件 | → [[app-activity-intent-redirect]] |

## Related

- [[app-activity-lifecycle]]
- [[app-provider-fileprovider-misconfig]]
- [[app-webview-file-access]]
- [[app-webview-js-bridge]]
- [[app-webview-url-bypass]]

- [[app-activity]]
- [[app-activity-intent-redirect]]
- [[app-intent]]
