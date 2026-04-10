# Fragment 注入

通过导出 Activity 动态加载攻击者指定的 Fragment，可能导致敏感操作或 UI 欺骗。

**Risk: MEDIUM**


## 利用前提

需要前置条件。导出 Activity 必须从 Intent extras 中读取类名并动态加载 Fragment，且未做类名白名单校验。攻击者需要知道可利用的 Fragment 类名（通常是应用内部类），且该 Fragment 存在敏感操作。

**Android 版本范围：所有版本可利用** — 应用层逻辑漏洞。


## 攻击流程

```
1. jiap code implement android.app.Fragment → 检查 Fragment 实现
2. jiap code subclass android.app.Fragment → 检查 Fragment 子类
3. jiap code class-source <Activity> → 搜索 getSupportFragmentManager/FragmentTransaction/replace/add
4. jiap ard exported-components → 定位导出 Activity
5. jiap code class-source <Activity> → 检查是否从 Intent 读取 Fragment 类名
6. jiap code subclass android.app.Fragment → 枚举应用内部包含敏感操作的 Fragment
7. adb shell am start -n com.target/.MainActivity --es fragment "com.target.AdminFragment"
8. 攻击者指定的 Fragment 被加载，显示管理界面或执行敏感操作
```


## 关键特征

- Activity 从 Intent extras 中读取 Fragment 类名
- 使用 `FragmentTransaction.replace/add` 动态加载
- 未校验 Fragment 类是否属于应用白名单


## 代码模式

```java
// 漏洞：动态加载 Intent 指定的 Fragment
public class MainActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        // 从 Intent 读取 Fragment 类名，无校验
        String fragmentName = getIntent().getStringExtra("fragment");
        if (fragmentName != null) {
            Fragment f = Fragment.instantiate(this, fragmentName);
            getSupportFragmentManager().beginTransaction()
                .replace(R.id.container, f).commit();
        }
    }
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2014-8609** | Android Settings 应用的 `PreferenceActivity.isValidFragment()` 未正确校验，攻击者可注入任意 Fragment 访问系统设置 |
| **CVE-2017-13156** | 通过 Fragment 注入结合 PreferenceActivity 获取设备管理员权限 |
| **CVE-2023-40088** | 第三方应用导出 Activity 未校验 Fragment 类名白名单，攻击者通过 Deep Link 注入包含敏感操作的内部 Fragment |


## 安全写法

```java
public class SecureMainActivity extends Activity {
    // Fragment 类名白名单
    private static final Set<String> ALLOWED_FRAGMENTS = Set.of(
        "com.example.app.HomeFragment",
        "com.example.app.ProfileFragment",
        "com.example.app.SettingsFragment"
    );

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        String fragmentName = getIntent().getStringExtra("fragment");
        if (fragmentName != null && ALLOWED_FRAGMENTS.contains(fragmentName)) {
            Fragment f = Fragment.instantiate(this, fragmentName);
            getSupportFragmentManager().beginTransaction()
                .replace(R.id.container, f).commit();
        }
    }
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Activity 导出访问 | 导出 Activity 可被远程 Deep Link 触发，无需安装应用即可注入 Fragment | → [[app-activity-exported-access]] |
| + StrandHogg | 劫持 Activity 后注入恶意 Fragment 显示钓鱼界面 | → [[app-activity-task-hijack]] |
| + WebView JS Bridge | 注入包含 WebView 的 Fragment，通过 JS Bridge 窃取数据 | → [[app-webview-js-bridge]] |
| + Intent 重定向 | Fragment 内读取 Activity Intent 数据并转发到其他组件 | → [[app-activity-intent-redirect]] |


## Related

- [[app-activity]]
- [[app-activity-task-hijack]]

