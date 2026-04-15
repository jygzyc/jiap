# ClassLoader 注入

通过 Intent extras 传递的序列化对象在反序列化时使用不受控的 ClassLoader，攻击者可指定任意类名实现代码执行。

**Risk: HIGH**


## 利用前提

独立可利用。目标代码使用 `getSerializableExtra()` / `readSerializable()` 反序列化外部传入数据，且 ClassLoader 不受控。攻击者通过导出组件或 AIDL 接口注入恶意类名即可触发代码执行。

**Android 版本范围：所有版本可利用** — 应用层漏洞，Android 14+ 加强了隐式 intent 限制但不影响导出组件/AIDL 场景。


## 攻击流程

```
1. decx code xref-method "android.os.Parcel.readSerializable()" → 搜索不安全的反序列化调用
2. decx code xref-method "android.os.Bundle.readSerializable()" → 追踪 Bundle.readSerializable 调用
3. decx code xref-method "android.os.Parcel.readValue()" → 追踪 readValue 调用
4. decx code xref-method "android.content.Intent.getSerializableExtra(...)" → 搜索 getSerializableExtra 调用
5. decx code class-source <Class> → 获取源码，追踪数据流
6. decx code xref-method "java.lang.ClassLoader.loadClass(...)" → 检查是否存在白名单校验
7. decx code xref-method "android.content.Intent.getSerializableExtra(...)" → 定位反序列化入口
8. 确认入口是否可被外部输入触发（导出组件、AIDL 接口）
9. 构造携带恶意类名的 Serializable/Parcelable 对象
10. 发送 Intent 触发目标代码的反序列化
11. ClassLoader 加载恶意类，触发静态初始化块或构造函数中的代码
```


## 关键特征与代码

- 使用 `Bundle.readSerializable()` / `Intent.getSerializableExtra()` / `Parcel.readSerializable()` 从外部输入反序列化，未指定 ClassLoader 或使用不受信任的 ClassLoader
- 反序列化后的对象被强制类型转换并调用方法，常出现在系统服务的 Bundle 处理中（AIDL 接口参数）

```java
// 漏洞：从 Intent extras 中反序列化，ClassLoader 不受控
public void handleIntent(Intent intent) {
    // getSerializableExtra 内部调用 readSerializable
    // readSerializable 使用 intent 的 ClassLoader 调用 loadClass(className)
    // 攻击者可通过 Intent extras 控制类名
    Serializable obj = intent.getSerializableExtra("config"); // ⚠️ 不安全
    Config config = (Config) obj; // ⚠️ 如果 className 被替换为恶意类，此处触发代码执行
    config.apply();
}

// Parcel 变体
protected Object readFromParcel(Parcel in) {
    Serializable obj = in.readSerializable(); // ⚠️ 使用默认 ClassLoader
    return obj;
}
```


## 经典案例

| 案例 | 攻击路径 |
|------|----------|
| **CVE-2023-20963** | Google Pixel Parcel Mismatch + ClassLoader 注入，用于野外利用链 (in-the-wild) |
| **CVE-2017-13286** | Android OutputConfiguration Parcelable 读写不对称，可注入任意类 |
| **CVE-2017-13288** | PeriodicAdvertisingReport Parcelable mismatch 导致 ClassLoader 注入 |


## 安全写法

```java
// 使用 Parcelable 替代 Serializable，不依赖 ClassLoader.loadClass
public class SafeConfig implements Parcelable {
    // writeToParcel / CREATOR 实现
}

// API 33+ 类型安全获取
SafeConfig config = intent.getParcelableExtra("config", SafeConfig.class);
```


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + Service AIDL | 通过 AIDL 接口传递恶意 Serializable 到系统服务 | → [[app-service-aidl-expose]] |
| + Parcel 读写不对称 | 构造恶意 Parcel 绕过类型校验后触发 ClassLoader 注入 | → [[app-intent-parcel-mismatch]] |
| + Framework 权限绕过 | 以系统身份反序列化恶意类，获得 system 权限代码执行 | → [[framework-service-permission-missing]] |


## Related

- [[app-intent]]
- [[app-service]]
