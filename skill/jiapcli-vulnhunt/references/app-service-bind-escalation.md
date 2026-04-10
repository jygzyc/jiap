# 绑定服务越权

应用绑定到系统或其他应用的 Service，利用该 Service 的权限执行本不应拥有的操作。

**Risk: HIGH**


## 利用前提

独立可利用。同 AIDL 暴露——Service 导出 + onBind 返回的 Binder 方法未校验调用者身份。任何应用绑定后直接调用。

**Android 版本范围：所有版本可利用** — 应用层配置问题。


## 攻击流程

```
1. jiap ard exported-components → 定位导出服务
2. jiap code xref-method "package.Class.bindService(...):boolean" → 搜索 bindService 调用点
3. jiap code implement android.content.ServiceConnection → 获取 ServiceConnection 实现类源码
4. jiap code class-source <ServiceClass> → 检查 onBind() 返回的 Binder 实现
5. 从 class-source 中定位 onServiceConnected 方法，检查获得的服务接口如何被使用
6. 枚举 Binder 公开方法，识别敏感操作（文件读写、数据库、命令执行）
7. 检查方法中是否有 getCallingUid/checkCallingPermission
8. 编写攻击应用 bindService() 调用目标方法
```


## 关键特征

- 应用绑定到系统服务的代理接口
- 通过绑定获得的服务接口执行特权操作
- 服务端未区分调用者身份
- `onBind` 返回的 Binder 接口未做权限校验
- **权限提升**：Service 内部执行了需要特定权限的敏感操作（如发送短信、安装应用），恶意应用无需申请该权限即可通过调用 Service 间接执行
- Manifest 中 `<service android:exported="true">` 但未设置 `android:permission`


## 代码模式

### 模式 1：文件操作越权

```java
// 漏洞：Service 未校验调用者身份，任意应用可绑定并调用敏感方法
public class FileManagerService extends Service {
    private final IBinder binder = new FileManagerBinder();

    public class FileManagerBinder extends Binder {
        public String readFile(String path) {
            // 未校验调用者，任意绑定应用可读取文件
            return FileUtils.read(path);
        }

        public void deleteFile(String path) {
            FileUtils.delete(path);
        }
    }

    @Override
    public IBinder onBind(Intent intent) {
        return binder;  // 直接返回，无校验
    }
}
```

### 模式 2：权限提升（敏感操作无鉴权）

```xml
<!-- 漏洞配置：导出且无权限保护 -->
<service android:name=".SMSService" android:exported="true" />
```

```java
// 漏洞：Service 持有 SEND_SMS 权限，但未校验调用者身份
public class SMSService extends Service {
    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String phone = intent.getStringExtra("phone");
        String content = intent.getStringExtra("content");
        // 发送短信需要 SEND_SMS 权限，恶意 App 通过调用此 Service 绕过自身权限缺失
        SmsManager.getDefault().sendTextMessage(phone, null, content, null, null);
        return START_NOT_STICKY;
    }
}

// 攻击代码：恶意 App 无需申请 SEND_SMS 权限即可发送短信
// Intent intent = new Intent();
// intent.setComponent(new ComponentName("com.victim", "com.victim.SMSService"));
// intent.putExtra("phone", "18888888888");
// intent.putExtra("content", "Hello");
// startService(intent);
```


## 攻击流程

```
1. jiap ard exported-components → 定位导出服务
2. jiap code class-source <ServiceClass> → 检查 onBind() 返回的 Binder 实现
3. 枚举 Binder 公开方法，识别敏感操作（文件读写、数据库、命令执行）
4. 检查方法中是否有 getCallingUid/checkCallingPermission
5. 编写攻击应用 bindService() 调用目标方法
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2021-0921** | Android 系统服务 Binder 接口未验证调用者，普通应用提权到 system_server |
| **CVE-2020-0108** | 绑定服务中的 Binder 接口允许恶意应用执行特权操作 |
| **银行 APP 服务接口** | 导出服务暴露了余额查询、转账接口无权限校验 |
| **猎豹清理大师** | 导出的 WidgetService 允许外部应用结束后台进程（权限泄漏） |
| **乐phone ApkInstaller** | 导出的 Service 允许任意应用安装/删除 APK |


## 安全写法

```java
public class SecureFileManagerService extends Service {
    private final IBinder binder = new FileManagerBinder();

    public class FileManagerBinder extends Binder {
        public String readFile(String path) {
            // 校验调用者身份
            int callingUid = Binder.getCallingUid();
            String[] packages = getPackageManager().getPackagesForUid(callingUid);
            if (!isAllowedPackage(packages)) {
                throw new SecurityException("Unauthorized caller: " + Arrays.toString(packages));
            }

            // 路径校验
            if (!isValidPath(path)) {
                throw new SecurityException("Invalid path");
            }

            return FileUtils.read(path);
        }
    }

    @Override
    public IBinder onBind(Intent intent) {
        // 额外在 onBind 中做一次校验
        enforceCallingPermission("com.example.PERMISSION_BIND");
        return binder;
    }

    private void enforceCallingPermission(String permission) {
        if (checkCallingOrSelfPermission(permission) != PackageManager.PERMISSION_GRANTED) {
            throw new SecurityException("Missing permission: " + permission);
        }
    }
}
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Framework 身份混淆 | 绑定系统服务后传入伪造的 userId，跨用户操作 | → [[framework-service-identity-confusion]] |
| + AIDL 接口暴露 | 通过绑定获取 AIDL 接口，调用多个未保护方法 | → [[app-service-aidl-expose]] |
| + Parcel 反序列化 | 绑定服务后发送恶意 Parcelable 触发反序列化漏洞 | → [[app-intent-parcel-mismatch]] |
| + Intent 重定向 | 服务接口返回数据被用于构造重定向 Intent | → [[app-activity-intent-redirect]] |
| + PendingIntent 滥用 | 利用受害 Service 的权限通过 PendingIntent 发送特权广播或启动特权组件 | → [[app-intent-pendingintent-escalation]] |


## Related

- [[app-service-messenger-abuse]]

- [[app-intent]]
- [[app-service]]
- [[framework-service-identity-confusion]]

