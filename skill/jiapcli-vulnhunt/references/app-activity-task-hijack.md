# Task 劫持（StrandHogg）

利用 `taskAffinity` 和 `allowTaskReparenting` 属性，恶意应用可将自身 Activity 插入目标应用的任务栈，实现 UI 欺骗。

**Risk: MEDIUM**


## 利用前提

独立可利用。目标 Activity 的 `taskAffinity` 为空或与恶意应用相同，且 launchMode 不是 `singleInstance`。攻击者发送带 `FLAG_ACTIVITY_NEW_TASK` 的 Intent 即可将恶意 Activity 插入目标任务栈。

**Android 版本范围：Android 10+ 可利用** — Android 11+ 增强了 taskAffinity 校验，Android 14+ 限制后台启动 Activity，攻击难度显著提升。


## 攻击流程

```
1. jiap ard app-manifest → 检查 manifest 中的任务栈相关属性
2. jiap code class-source <Activity> → 搜索 setTaskAffinity/setTaskReparenting 调用
3. jiap code xref-field "android:taskAffinity" → 读取 manifest 中的静态配置

StrandHogg 1.0：
4. 恶意 App 设置 taskAffinity 为目标应用包名（如 com.google.android.gm）
5. 用户打开恶意 Activity 后按 Home 键返回桌面
6. 用户再次打开目标应用时，恶意 Activity 位于栈顶，显示伪造界面

StrandHogg 2.0（CVE-2020-0096）：
4. 利用 ActivityStarter 的 AUTOMERGE 特性
5. 通过特定 flag 组合使系统将恶意任务栈与目标应用任务栈合并
6. 无需目标应用声明特殊 affinity
```


## 关键特征与代码

- Activity 声明 `android:taskAffinity=""`（空 affinity）或与目标应用相同的 affinity，设置 `android:allowTaskReparenting="true"`，使用 `singleTask` 或 `singleInstance` launchMode，未设置 `android:excludeFromRecents`

```xml
<!-- 漏洞：taskAffinity 为空，可被恶意应用利用 -->
<activity
    android:name=".LoginActivity"
    android:exported="true"
    android:taskAffinity=""
    android:launchMode="singleTask"
    android:allowTaskReparenting="true" />
```

```java
// 漏洞：代码中动态设置 taskAffinity
Intent intent = new Intent(this, TargetActivity.class);
intent.addFlags(0x10000000);  // 0x10000000 = FLAG_ACTIVITY_NEW_TASK
intent.addFlags(0x20000);     // 0x20000 = FLAG_ACTIVITY_REORDER_TO_FRONT
startActivity(intent);
// 如果 taskAffinity 为空或可预测，恶意应用可注入同 affinity 的 Activity
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2020-0096 (StrandHogg 2.0)** | 利用 ActivityStarter 的 AUTOMERGE 特性，通过特定 flag 组合将恶意任务栈与目标应用合并，无需目标声明特殊 affinity |
| **CVE-2019-2114** | 恶意应用通过 taskAffinity 劫持银行应用任务栈，伪造登录界面窃取用户凭证 |

## 安全写法

```xml
<!-- 修复：设置 taskAffinity 为自身包名，使用 singleTask launchMode -->
<activity
    android:name=".LoginActivity"
    android:exported="true"
    android:taskAffinity="com.example.myapp"
    android:launchMode="singleTask"
    android:excludeFromRecents="false" />
```

```java
// 运行时校验：验证任务栈完整性
@Override
protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);
    // 检查当前任务栈是否包含预期的 Activity
    ActivityManager am = (ActivityManager) getSystemService(ACTIVITY_SERVICE);
    List<ActivityManager.AppTask> tasks = am.getAppTasks();
    if (tasks.isEmpty()) {
        finish();
        return;
    }
    // 校验任务栈顶层是否属于自身应用
    ComponentName topActivity = tasks.get(0).getTaskInfo().topActivity;
    if (topActivity == null || !topActivity.getPackageName().equals(getPackageName())) {
        Log.w("Security", "Task stack tampered");
        finish();
        return;
    }
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + JS Bridge 窃取 | 劫持 WebView 登录页面，伪造表单 + JS Bridge 窃取凭证 | → [[app-webview-js-bridge]] |
| + Fragment 注入 | 劫持后注入恶意 Fragment 显示钓鱼界面 | → [[app-activity-fragment-injection]] |
| + setResult 泄露 | 劫持 Activity 后截获目标应用通过 setResult 返回的敏感数据 | → [[app-activity-setresult-leak]] |
| + 点击劫持 | 任务栏伪造界面配合覆盖层，双重欺骗 | → [[app-activity-clickjacking]] |
| + 生命周期处理 | 劫持任务栏后目标 Activity 后台资源未释放，持续监听 | → [[app-activity-lifecycle]] |


## Related

- [[app-activity]]
- [[app-activity-clickjacking]]
- [[app-activity-fragment-injection]]
- [[app-activity-lifecycle]]
- [[app-activity-setresult-leak]]
- [[app-webview]]
- [[app-webview-js-bridge]]
