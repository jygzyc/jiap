# 系统服务数据泄露

系统服务返回不应暴露给普通应用的数据，未用 `Binder.getCallingUid()` 或 `enforceCallingPermission()` 校验调用者身份。**Risk: HIGH**


## 利用前提

独立可利用。系统服务返回了不应暴露给普通应用的数据（其他应用的配置、系统内部状态等），未校验调用者身份。任何应用通过 Binder 即可获取。

**Android 版本范围：视具体系统服务而定** — 取决于目标 Android 版本的系统服务实现，AI 扫描时应标注发现的 Android 版本。


## 攻击流程

```
1. jiap ard system-service-impl <Interface> → 定位系统服务实现
2. jiap code class-source <ServiceImpl> → 获取实现类源码
3. jiap code xref-method "<ServiceImpl.getRunningAppProcesses(...)>" → 追踪 getRunningAppProcesses 调用
4. jiap code xref-method "<ServiceImpl.getRecentTasks(...)>" → 追踪 getRecentTasks 调用
5. jiap code xref-method "<ServiceImpl.getInstalledPackages(...)>" → 追踪 getInstalledPackages 调用
6. jiap code xref-method "<ServiceImpl.getAccounts(...)>" → 追踪 getAccounts 调用
7. jiap code xref-method "<ServiceImpl.getDeviceId(...)>" → 追踪 getDeviceId 调用
8. jiap code xref-method "<ServiceImpl.getSerial(...)>" → 追踪 getSerial 调用
9. jiap code xref-method "<ServiceImpl.getWifiInfo(...)>" → 追踪 getWifiInfo 调用
10. jiap code class-source <ServiceImpl> → 识别返回数据的方法
11. 检查返回内容是否包含敏感信息（设备ID、应用列表、用户信息）
12. 确认缺少 enforceCallingPermission 校验
13. 编写普通应用通过 Binder 调用目标接口获取数据
```


## 关键特征

- 方法返回设备信息、用户信息、其他应用信息
- 未调用 `Binder.getCallingUid()` / `enforceCallingPermission()` 做权限检查
- 返回完整数据而非调用者应访问的子集
- 方法参数中缺少 `String callingPackage` 等来源标识


## 代码模式

```java
// 漏洞：系统服务返回敏感数据未校验调用者
public class DeviceInfoService extends IDeviceInfoService.Stub {
    @Override
    public Bundle getDeviceInfo() {
        // 无权限校验，任何应用均可获取
        Bundle info = new Bundle();
        info.putString("device_id", getDeviceId());
        info.putString("serial", Build.getSerial());  // 设备序列号
        info.putString("wifi_mac", getWifiMacAddress()); // MAC 地址
        info.putString("imei", getImei()); // IMEI
        return info; // 敏感设备标识符泄露
    }
}
```


## 攻击流程

```
1. jiap ard system-service-impl <Interface> → 定位系统服务实现
2. jiap code class-source <ServiceImpl> → 识别返回数据的方法
3. 检查返回内容是否包含敏感信息（设备ID、应用列表、用户信息）
4. 确认缺少 enforceCallingPermission 校验
5. 编写普通应用通过 Binder 调用目标接口获取数据
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2021-0390** | LocationManager 服务未过滤位置数据，普通应用可获取精确位置 |
| **CVE-2023-21244** | 系统服务泄露包安装状态信息，绕过隐藏应用检测 |
| **CVE-2022-20428** | Settings 服务返回受限设置信息，普通应用可读取其他用户配置 |
| **CVE-2024-0031** | Telephony 服务泄露 IMEI/SN 等设备标识符给无权限应用 |
| **CVE-2024-43090** | Keyboard shortcuts helper 泄露 Icon 对象 |


## 安全写法

```java
public class SecureDeviceInfoService extends IDeviceInfoService.Stub {
    @Override
    public Bundle getDeviceInfo() {
        // 强制权限校验
        getContext().enforceCallingOrSelfPermission(
                "android.permission.READ_PRIVILEGED_DEVICE_INFO",
                "Requires READ_PRIVILEGED_DEVICE_INFO permission");
        // 或校验调用者 UID
        int callingUid = Binder.getCallingUid();
        if (!isSystemOrPrivileged(callingUid)) {
            throw new SecurityException("Caller uid=" + callingUid + " not privileged");
        }
        Bundle info = new Bundle();
        info.putString("device_id", getDeviceId());
        return info;
    }
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + 隐式 Intent 劫持 | 泄露的包名/组件信息用于构造精准的隐式 Intent 攻击 | → [[app-intent-implicit-hijack]] |
| + Intent 重定向 | 泄露的特权组件信息用于构造 Intent 重定向攻击 | → [[framework-service-intent-redirect]] |
| + 权限检查缺失 | 数据泄露方法本身就缺少权限检查 | → [[framework-service-permission-missing]] |
| + Broadcast 本地泄露 | 系统广播中携带敏感信息被任意应用监听 | → [[app-broadcast-local-leak]] |


## Related

- [[framework-service-identity-confusion]]

- [[app-intent-implicit-hijack]]
- [[framework-service]]

