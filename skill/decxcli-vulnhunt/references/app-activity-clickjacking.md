# 点击劫持（Clickjacking）

利用 `SYSTEM_ALERT_WINDOW` 权限覆盖系统对话框（权限请求、卸载确认），诱骗用户点击。

**Risk: LOW**


## 利用前提

需要前置条件。①恶意应用必须拥有 `SYSTEM_ALERT_WINDOW` 权限（需用户手动授予）；②需要用户交互（点击）；③目标 Activity 处理敏感操作（确认支付、授权等）。单独的点击劫持危害有限，常作为社工链的一环。

**Android 版本范围：所有版本可利用** — Android 14+ 限制后台启动 Activity，但核心防御仍依赖开发者设置 `filterTouchesWhenObscured`。


## 攻击流程

```
1. decx ard app-manifest → 检查 manifest 和权限
2. decx code xref-field "android.Manifest.permission.SYSTEM_ALERT_WINDOW" → 检查触摸安全相关
3. decx code xref-field "android.view.WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY" → 检查覆盖层使用
4. decx code subclass android.app.Activity → 检查是否覆写了 onFilterTouchEventForSecurity
5. 恶意应用申请 SYSTEM_ALERT_WINDOW 权限（引导用户授予）
6. 监听目标应用的敏感对话框操作（权限请求、支付确认、卸载确认）
7. 在对话框弹出时覆盖透明 Layer，显示误导性 UI
8. 用户点击覆盖层上的伪造按钮，实际触发了底层敏感操作的确认
9. 攻击者获得用户无意授权的权限/操作
```


## 关键特征与代码

- 目标 Activity 未设置 `android:filterTouchesWhenObscured="true"`
- 系统对话框（权限授予、设备管理器激活、安装确认）可被覆盖
- 未覆写 `onFilterTouchEventForSecurity` 检测覆盖
- 敏感操作按钮（授权、确认）无额外二次验证

```java
// 漏洞：未设置 filterTouchesWhenObscured，覆盖层可欺骗点击
public class SettingsActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_settings);

        // 敏感操作：授予设备管理器权限
        btnGrantAdmin.setOnClickListener(v -> {
            startActivity(new Intent(DevicePolicyManager.ACTION_ADD_DEVICE_ADMIN));
        });
    }
}
```


## 安全写法

```xml
<!-- Manifest 中声明 filterTouchesWhenObscured -->
<activity android:name=".SettingsActivity"
    android:filterTouchesWhenObscured="true" />
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2020-0306** | 覆盖蓝牙发现请求确认框，诱导用户同意蓝牙配对 |
| **CVE-2021-0314** | 覆盖卸载确认对话框，诱骗用户确认卸载安全软件 |


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + 前台服务 + 覆盖通知 | 覆盖通知栏，拦截或伪造通知中的操作 | → [[app-service-foreground-leak]] |
| + 任务栈劫持 | 覆盖层 + StrandHogg 双重欺骗，伪造完整应用界面 | → [[app-activity-task-hijack]] |
| + 组件导出越权 | 覆盖层误导用户操作导出的敏感 Activity，完成越权 | → [[app-activity-exported-access]] |
| + PendingIntent 滥用 | 覆盖权限请求对话框后，用户无意授权 PendingIntent 操作 | → [[app-activity-pendingintent-abuse]] |


## Related

- [[app-activity]]
- [[app-intent]]

