# 生命周期处理不当

敏感操作（摄像头、录音、定位）未在正确生命周期回调中关闭，攻击者可构造任务栈使目标应用在后台持续执行。

**Risk: MEDIUM**


## 利用前提

需要组合。`onSaveInstanceState` 数据泄露需要配合 ADB backup 提取或 root 权限读取 `/data/data/` 下的 saved_state 文件。`onPause` 资源未清理则需要恶意应用在目标 Activity 进入后台时快速操作。

**Android 版本范围：Android 10+ 可利用** — Android 12+ 限制了 `adb backup`，但 `onPause` 资源未释放问题不受系统版本影响。


## 攻击流程

```
1. jiap code class-source <Activity> → 获取 Activity 源码，检查生命周期和资源管理
2. 搜索 openCamera/startRecording/requestLocationUpdates/registerListener
3. 检查 onPause/onStop/onDestroy 中的 release/stopRecording/unregisterListener
4. 目标 Activity 在 onCreate 中打开摄像头推流
5. 释放逻辑仅在 onDestroy 中（而非 onPause）
6. 攻击者构造任务栈：目标 Activity → 恶意 Activity（置于前台）
7. 目标 Activity 进入后台但未被销毁
8. 摄像头持续推流，用户不知情
```


## 关键特征与代码

- 在 `onCreate`/`onResume` 中启动敏感操作，但仅在 `onDestroy` 中释放，缺少 `onPause`/`onStop` 中的资源释放逻辑
- `onSaveInstanceState` 中保存敏感数据（Token、密码等），可被提取

```java
// 漏洞1：敏感资源未在 onPause 中释放
public class CameraActivity extends Activity {
    @Override
    protected void onResume() {
        super.onResume();
        camera = Camera.open(0);  // 启动摄像头
        camera.startPreview();
    }

    @Override
    protected void onDestroy() {
        camera.release();  // 仅在 onDestroy 释放，后台时仍运行
    }
}

// 漏洞2：onSaveInstanceState 保存敏感数据
@Override
protected void onSaveInstanceState(Bundle outState) {
    super.onSaveInstanceState(outState);
    outState.putString("auth_token", authToken);  // 敏感数据泄露到 Bundle
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2019-2234** | Google Camera 在后台未释放摄像头资源，恶意应用可在用户无感知情况下拍照和录像 |
| **CVE-2020-0381** | 多款银行 APP 在 onSaveInstanceState 中保存用户凭证，通过 adb backup 可提取 |


## 安全写法

```java
// 修复1：在 onPause 中释放敏感资源
public class CameraActivity extends Activity {
    @Override
    protected void onPause() {
        super.onPause();
        if (camera != null) {
            camera.stopPreview();
            camera.release();
            camera = null;
        }
    }
}

// 修复2：不在 onSaveInstanceState 中保存敏感数据
@Override
protected void onSaveInstanceState(Bundle outState) {
    super.onSaveInstanceState(outState);
    // 不保存 auth_token，从安全存储（如 EncryptedSharedPreferences）恢复
    outState.putString("last_screen", screenId);  // 仅保存非敏感 UI 状态
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + 任务栈劫持 | 劫持任务栈后，目标 Activity 进入后台但资源未释放，持续监听 | → [[app-activity-task-hijack]] |
| + 数据泄露 | onSaveInstanceState 保存的敏感数据在 Activity 重建时暴露 | → [[app-activity-setresult-leak]] |
| + 组件导出越权 | 导出 Activity 的异常生命周期服务未终止，持续以用户身份运行 | → [[app-activity-exported-access]] |


## Related

- [[app-activity]]
