# 系统服务安全审计

系统服务运行于 system_server 或特权进程，拥有高权限。身份混淆、权限检查缺失、Intent 重定向等漏洞可导致本地提权、数据泄露和系统服务崩溃。

## 风险清单

| 风险 | 等级 | 详情 |
|------|------|------|
| clearCallingIdentity 滥用 | HIGH | [[framework-service-clear-identity]] |
| 权限检查缺失 | HIGH | [[framework-service-permission-missing]] |
| 身份混淆 | HIGH | [[framework-service-identity-confusion]] |
| Intent 重定向 | HIGH | [[framework-service-intent-redirect]] |
| 数据泄露 | MEDIUM | [[framework-service-data-leak]] |
| 竞态条件 | HIGH | [[framework-service-race-condition]] |

## 分析流程

```
1. decx ard system-service-impl "<Interface>" -P <port> → 定位系统服务实现
2. decx code class-source "<ServiceImpl>" -P <port>     → 获取实现类源码
3. 检查 Binder 调用模式：
   a. 搜索 clearCallingIdentity / restoreCallingIdentity → 身份操作
   b. 搜索 enforceCallingPermission / checkCallingPermission → 权限检查
   c. 搜索 getCallingUid / getCallingPid → 身份获取
4. 检查 Intent 转发：
   decx code xref-method "android.content.Context.startActivity(android.content.Intent):void" -P <port>
   → 系统服务中以 system 身份启动 Intent 的调用
5. 检查数据返回：
   decx code xref-method "android.os.Binder.getCallingUid():int" -P <port>
   → 基于 UID 的权限判断逻辑
6. 检查竞态条件：
   同步块范围、锁粒度、异步回调中的状态一致性
```

## 关键追踪模式

- **clearCallingIdentity**：clear/restore 之间的代码以 system 身份执行，检查范围
- **权限缺失**：公开 API 方法未调用 `enforceCallingPermission`
- **身份混淆**：`getCallingUid()` 缓存后跨 Binder 调用使用，身份可能改变
- **Intent 重定向**：以 system 权限启动攻击者控制的 Intent
- **竞态条件**：多线程访问共享状态缺少同步

## Related

[[app-activity]]
[[app-intent]]
[[app-service]]
[[app-provider]]
[[app-broadcast]]
[[risk-rating]]
