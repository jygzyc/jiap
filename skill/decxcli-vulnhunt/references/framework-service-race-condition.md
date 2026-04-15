# 条件竞争（Race Condition）

系统服务中的并发操作未正确同步，可能导致双重操作或状态不一致。

**Risk: MEDIUM → HIGH**

利用竞态窗口实现双重操作或状态不一致。需要精确的时序控制。


## 利用前提

独立可利用（但利用难度高）。需要精确的时序控制才能触发竞态窗口。通常通过并发发送大量请求来增大命中概率。危害取决于竞态窗口内可执行的操作（双重领取、权限状态不一致等）。

**Android 版本范围：所有版本可利用** — 竞态条件是逻辑问题，系统层面无法完全修复。但利用难度高，需要精确时序控制。


## 攻击流程

```
1. decx ard system-service-impl <Interface> → 定位系统服务实现
2. decx code class-source <ServiceImpl> → 获取实现类源码
3. 在源码中检查同步机制：
   → 搜索 synchronized, ReentrantLock, AtomicInteger 等并发控制关键字
4. decx code class-source <ServiceImpl> → 搜索「检查-操作」模式
5. 检查是否缺少 synchronized/Lock 保护
6. 识别非线程安全数据结构（HashMap vs ConcurrentHashMap）
7. 并发发送大量 Binder 调用尝试触发竞态窗口
```


## 关键特征与代码

- 检查-操作模式未使用同步保护（`synchronized` / `Lock` / `AtomicReference`）
- 使用非线程安全的数据结构（`HashMap` 而非 `ConcurrentHashMap`）
- 文件操作没有原子性保证

```java
// 漏洞：检查-操作无同步保护
public boolean transferCredits(String from, String to, int amount) {
    if (getBalance(from) >= amount) {          // 检查
        // 另一个线程可能在这里也通过了检查
        deduct(from, amount);                   // 操作
        credit(to, amount);
        return true;
    }
    return false;
}
```


## 经典案例

| 案例 | 攻击场景 |
|------|----------|
| **CVE-2021-0920** | Linux 内核 io_uring 竞态条件，可被 Android 本地应用利用提权 |
| **CVE-2022-25667** | Binder 驱动竞态条件导致 UAF，实现本地提权 |
| **双重领取** | 应用内购服务竞态条件，重复领取奖励 |


## 安全写法

```java
// ✅ 正确：synchronized + double-check 防止竞态
private final Object mLock = new Object();

public boolean transferCredits(String from, String to, int amount) {
    synchronized (mLock) {
        if (getBalanceLocked(from) >= amount) {
            deductLocked(from, amount);
            creditLocked(to, amount);
            return true;
        }
        return false;
    }
}

// ✅ 正确：AtomicReference 保证复合操作的原子性
private final AtomicReference<AccountState> mState =
    new AtomicReference<>(new AccountState(0, false));

public boolean claimBonus(int bonus) {
    AccountState current, next;
    do {
        current = mState.get();
        if (current.claimed) {
            return false; // 已领取
        }
        next = new AccountState(current.balance + bonus, true);
    } while (!mState.compareAndSet(current, next));
    return true;
}

// ✅ 正确：文件操作使用原子写入
public void saveConfig(File file, String content) {
    File tmp = new File(file.getPath() + ".tmp");
    try {
        Files.writeString(tmp.toPath(), content);
        if (!tmp.renameTo(file)) {
            throw new IOException("rename failed");
        }
        // rename 在同一文件系统上是原子操作
    } catch (IOException e) {
        tmp.delete();
        throw new RuntimeException("Failed to save config", e);
    }
}
```

**核心原则：** 对共享状态的检查-操作必须用 `synchronized` 或 CAS（`AtomicReference.compareAndSet`）保护。文件操作应使用写入临时文件 + rename 的原子模式。


## 关联漏洞与组合利用

| 组合 | 链条效果 | 参考文件 |
|------|----------|----------|
| + 身份混淆 | 竞态窗口内伪造身份，通过 userId 参数越权 | → [[framework-service-identity-confusion]] |
| + 权限缺失 | 并发请求绕过 checkCallingPermission 后执行特权操作 | → [[framework-service-permission-missing]] |
| + clearIdentity 滥用 | clearCallingIdentity 范围内存在竞态，操作身份混乱 | → [[framework-service-clear-identity]] |


## Related

- [[framework-service]]
