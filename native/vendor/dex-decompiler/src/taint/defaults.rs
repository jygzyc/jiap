//! Bundled default Android models / rules (Mariana Trench–inspired kinds).

use super::config::TaintConfig;

/// Default configuration embedded in the binary.
pub fn default_config() -> TaintConfig {
    TaintConfig::from_json_str(DEFAULT_JSON).expect("embedded default taint config must parse")
}

const DEFAULT_JSON: &str = r#"
{
  "sources": [
    {"patterns": ["getIntent", "Activity.getIntent"], "port": "return", "kind": "ActivityUserInput", "features": ["user-controlled"]},
    {"patterns": ["Intent.getStringExtra", "Intent.getStringArrayExtra", "Intent.getStringArrayListExtra", "Intent.getCharSequenceExtra", "Intent.getCharSequenceArrayExtra", "Intent.getParcelableExtra", "Intent.getParcelableArrayExtra", "Intent.getParcelableArrayListExtra", "Intent.getSerializableExtra", "Intent.getBundleExtra", "Intent.getExtras", "Intent.getDataString", "Intent.getData", "Intent.getClipData"], "port": "return", "kind": "ActivityUserInput"},
    {"patterns": ["Bundle.getString", "Bundle.getStringArray", "Bundle.getStringArrayList", "Bundle.getCharSequence", "Bundle.getParcelable", "Bundle.getParcelableArray", "Bundle.getParcelableArrayList", "Bundle.getSerializable", "Bundle.getBundle", "PersistableBundle.getString", "PersistableBundle.getStringArray", "PersistableBundle.getPersistableBundle"], "port": "return", "kind": "ActivityUserInput"},
    {"patterns": ["getQueryParameter", "Uri.getQueryParameter", "getLastPathSegment", "getPath"], "port": "return", "kind": "ActivityUserInput"},
    {"patterns": ["getParcelableExtra", "getParcelableArrayExtra", "getParcelableArrayListExtra"], "port": "return", "kind": "NestedIntent"},
    {"patterns": ["EditText.getText"], "port": "return", "kind": "UserInput"},
    {"patterns": ["ClipboardManager.getPrimaryClip", "ClipboardManager.getText", "getPrimaryClip"], "port": "return", "kind": "Clipboard"},
    {"patterns": ["getDeviceId", "getImei", "getSubscriberId", "getAndroidId", "Settings$Secure.getString"], "port": "return", "kind": "DeviceId"},
    {"patterns": ["getLastLocation", "getCurrentLocation", "getLatitude", "getLongitude"], "port": "return", "kind": "Location"},
    {"patterns": [".onReceive("], "port": {"argument": {"index": 2}}, "kind": "ReceiverUserInput"},
    {"patterns": [".onNewIntent(", ".onStartCommand("], "port": {"argument": {"index": 1}}, "kind": "ActivityUserInput"},
    {"patterns": [".onActivityResult("], "port": {"argument": {"index": 3}}, "kind": "ActivityUserInput"},
    {"patterns": [".onBind("], "port": {"argument": {"index": 1}}, "kind": "ActivityUserInput"},
    {"patterns": ["ContentResolver.query"], "port": "return", "kind": "ProviderUserInput"},
    {"patterns": ["getInstalledPackages", "createPackageContext", "getApplicationInfo", "File.getAbsolutePath", "Environment.getExternalStorageDirectory"], "port": "return", "kind": "UntrustedCodePath"}
  ],
  "sinks": [
    {"patterns": ["Runtime.exec", "ProcessBuilder.<init>", "ProcessBuilder.start"], "port": {"argument": {"index": 0}}, "kind": "CodeExecution"},
    {"patterns": ["DexClassLoader.<init>", "PathClassLoader.<init>", "InMemoryDexClassLoader.<init>"], "port": {"argument": {"index": 1}}, "kind": "CodeExecution"},
    {"patterns": ["loadClass", "System.load", "System.loadLibrary", "Method.invoke", "Class.forName"], "port": {"argument": {"index": 1}}, "kind": "CodeExecution"},
    {"patterns": ["rawQuery", "execSQL", "compileStatement"], "port": {"argument": {"index": 1}}, "kind": "SQLQuery"},
    {"patterns": ["WebView.loadUrl", "loadData", "loadDataWithBaseURL", "evaluateJavascript"], "port": {"argument": {"index": 1}}, "kind": "ExecuteJavascript"},
    {"patterns": ["addJavascriptInterface"], "port": {"argument": {"index": 1}}, "kind": "JavascriptInterface"},
    {"patterns": ["setAllowFileAccessFromFileURLs", "setAllowUniversalAccessFromFileURLs"], "port": {"argument": {"index": 1}}, "kind": "WebViewFileAccess"},
    {"patterns": ["Log.d", "Log.i", "Log.w", "Log.e", "Log.v", "println"], "port": {"argument": {"index": 1}}, "kind": "Logging"},
    {"patterns": ["startActivity", "startActivityForResult", "startService", "bindService", "sendBroadcast", "sendOrderedBroadcast"], "port": {"argument": {"index": 1}}, "kind": "LaunchingComponent"},
    {"patterns": ["setResult"], "port": {"argument": {"index": 2}}, "kind": "SetResult"},
    {"patterns": ["Intent.setClipData", "setClipData", "Intent.addFlags", "addFlags"], "port": {"argument": {"index": 1}}, "kind": "UriGrant"},
    {"patterns": ["grantUriPermission", "takePersistableUriPermission", "ContentResolver.takePersistableUriPermission"], "port": {"argument": {"index": 2}}, "kind": "UriGrant"},
    {"patterns": ["openConnection", "HttpURLConnection", "OkHttpClient", "Request.Builder.url", "Call.execute", "Call.enqueue", "OutputStream.write", "URLConnection.getOutputStream", "HttpURLConnection.getOutputStream"], "port": {"argument": {"index": 0}}, "kind": "Network"},
    {"patterns": ["OutputStream.write", "BufferedWriter.write", "Writer.write", "RequestBody.create"], "port": {"argument": {"index": 1}}, "kind": "Network"},
    {"patterns": ["CookieManager.setCookie", "setCookie"], "port": {"argument": {"index": 1}}, "kind": "CookieWrite"},
    {"patterns": ["CookieManager.getCookie", "getCookie"], "port": {"argument": {"index": 1}}, "kind": "CookieRead"},
    {"patterns": ["CustomTabsIntent.launchUrl", "launchUrl"], "port": {"argument": {"index": 2}}, "kind": "CustomTabsLaunch"},
    {"patterns": ["ClipboardManager.setPrimaryClip", "ClipboardManager.setText", "setPrimaryClip"], "port": {"argument": {"index": 1}}, "kind": "ClipboardWrite"},
    {"patterns": ["setHostnameVerifier", "hostnameVerifier", "HostnameVerifier.verify", "checkServerTrusted"], "port": {"argument": {"index": 1}}, "kind": "SslBypass"},
    {"patterns": ["SSLContext.init"], "port": {"argument": {"index": 2}}, "kind": "SslBypass"},
    {"patterns": ["FileOutputStream.<init>", "FileWriter.<init>", "openFileOutput", "ParcelFileDescriptor.open"], "port": {"argument": {"index": 1}}, "kind": "FileWrite"},
    {"patterns": ["java.io.File.<init>", "File.<init>"], "port": {"argument": {"index": 2}}, "kind": "FileWrite"},
    {"patterns": ["java.io.File.<init>", "File.<init>"], "port": {"argument": {"index": 1}}, "kind": "FileWrite"},
    {"patterns": ["SharedPreferences$Editor.putString", "Editor.putString"], "port": {"argument": {"index": 2}}, "kind": "SharedPrefsWrite"},
    {"patterns": ["ObjectInputStream.readObject", "readObject"], "port": {"argument": {"index": 0}}, "kind": "Deserialization"},
    {"patterns": ["SecretKeySpec.<init>", "IvParameterSpec.<init>"], "port": {"argument": {"index": 1}}, "kind": "WeakCrypto"}
  ],
  "propagations": [
    {"patterns": ["StringBuilder.append", "StringBuffer.append"], "from": {"argument": {"index": 1}}, "to": {"argument": {"index": 0}}},
    {"patterns": ["StringBuilder.toString", "StringBuffer.toString"], "from": {"argument": {"index": 0}}, "to": "return"},
    {"patterns": ["String.valueOf", "String.copyValueOf", "String.concat", "String.substring", "String.subSequence", "String.trim", "String.toLowerCase", "String.toUpperCase", "String.replace", "String.getBytes", "CharSequence.toString"], "from": {"argument": {"index": 0}}, "to": "return"},
    {"patterns": ["Arrays.copyOf", "Arrays.copyOfRange", "Collections.unmodifiableList", "Collections.unmodifiableMap", "Objects.requireNonNull"], "from": {"argument": {"index": 0}}, "to": "return"},
    {"patterns": ["Uri.parse", "Uri.Builder.build", "Uri.Builder.appendQueryParameter"], "from": {"argument": {"index": 0}}, "to": "return"},
    {"patterns": ["Intent.putExtra", "putExtra"], "from": {"argument": {"index": 2}}, "to": {"argument": {"index": 0}}},
    {"patterns": ["Intent.setData", "setData", "setDataAndType"], "from": {"argument": {"index": 1}}, "to": {"argument": {"index": 0}}},
    {"patterns": ["Intent.setClipData", "setClipData"], "from": {"argument": {"index": 1}}, "to": {"argument": {"index": 0}}},
    {"patterns": ["Intent.addFlags", "addFlags", "Intent.setFlags", "setFlags"], "from": {"argument": {"index": 1}}, "to": {"argument": {"index": 0}}},
    {"patterns": ["Bundle.putString", "Bundle.putStringArray", "Bundle.putStringArrayList", "Bundle.putCharSequence", "Bundle.putParcelable", "Bundle.putParcelableArray", "Bundle.putParcelableArrayList", "Bundle.putSerializable", "Bundle.putBundle", "PersistableBundle.putString", "PersistableBundle.putStringArray", "PersistableBundle.putPersistableBundle"], "from": {"argument": {"index": 2}}, "to": {"argument": {"index": 0}}},
    {"patterns": ["ClipData.newRawUri", "ClipData.newUri", "ClipData.Item.getUri"], "from": {"argument": {"index": 1}}, "to": "return"},
    {"patterns": ["setLoginUrl"], "from": {"argument": {"index": 1}}, "to": {"argument": {"index": 0}}},
    {"patterns": ["Cipher.doFinal", "javax.crypto.Cipher.doFinal"], "from": {"argument": {"index": 1}}, "to": "return"},
    {"patterns": ["Cipher.update", "javax.crypto.Cipher.update"], "from": {"argument": {"index": 1}}, "to": "return"},
    {"patterns": ["CipherOutputStream.write"], "from": {"argument": {"index": 1}}, "to": {"argument": {"index": 0}}},
    {"patterns": ["MessageDigest.digest", "MessageDigest.update"], "from": {"argument": {"index": 1}}, "to": "return"},
    {"patterns": ["Base64.encode", "Base64.encodeToString"], "from": {"argument": {"index": 0}}, "to": "return"}
  ],
  "sanitizers": [
    {"patterns": ["MessageDigest.digest", "MessageDigest.update", "hashCode", "Objects.hash"], "kinds": []},
    {"patterns": ["URLEncoder.encode", "Uri.encode"], "kinds": []},
    {"patterns": ["Base64.encode", "Base64.encodeToString"], "kinds": []}
  ],
  "rules": [
    {
      "name": "User input to code execution (RCE)",
      "code": 1,
      "description": "User-/intent-controlled values may flow into a code-execution sink",
      "sources": ["ActivityUserInput", "UserInput", "ReceiverUserInput", "ProviderUserInput", "UntrustedCodePath"],
      "sinks": ["CodeExecution"]
    },
    {
      "name": "User input to SQL",
      "code": 4,
      "description": "User-controlled values may flow into a raw SQL statement",
      "sources": ["ActivityUserInput", "UserInput", "ReceiverUserInput", "ProviderUserInput"],
      "sinks": ["SQLQuery"]
    },
    {
      "name": "User input to WebView / JS",
      "code": 5,
      "description": "User-controlled values may flow into WebView script execution",
      "sources": ["ActivityUserInput", "UserInput", "ReceiverUserInput"],
      "sinks": ["ExecuteJavascript", "JavascriptInterface", "WebViewFileAccess"]
    },
    {
      "name": "User input to intent launch",
      "code": 3,
      "description": "User-controlled values may flow into an intent launcher",
      "sources": ["ActivityUserInput", "UserInput", "ReceiverUserInput"],
      "sinks": ["LaunchingComponent", "SetResult", "UriGrant"]
    },
    {
      "name": "PII to logging",
      "code": 10,
      "description": "Device identifiers or location may flow into logs",
      "sources": ["DeviceId", "Location", "Clipboard", "ActivityUserInput", "UserInput"],
      "sinks": ["Logging", "Network", "ClipboardWrite", "CookieWrite"]
    },
    {
      "name": "User input to network",
      "code": 11,
      "description": "User-controlled values may flow into network APIs",
      "sources": ["ActivityUserInput", "UserInput"],
      "sinks": ["Network", "CookieWrite"]
    },
    {
      "name": "User input to file write",
      "code": 12,
      "description": "User-controlled values may flow into file write APIs",
      "sources": ["ActivityUserInput", "UserInput", "ProviderUserInput"],
      "sinks": ["FileWrite"]
    },
    {
      "name": "User input to shared preferences",
      "code": 13,
      "description": "User-controlled values may be written into SharedPreferences",
      "sources": ["ActivityUserInput", "UserInput"],
      "sinks": ["SharedPrefsWrite"]
    },
    {
      "name": "User input to deserialization",
      "code": 14,
      "description": "User-controlled values may reach unsafe deserialization",
      "sources": ["ActivityUserInput", "UserInput", "ReceiverUserInput"],
      "sinks": ["Deserialization"]
    },
    {
      "name": "User input to URI grant / ClipData flags",
      "code": 15,
      "description": "User-controlled Intent/URI may receive FLAG_GRANT_* or ClipData URI grants via setResult/setClipData/addFlags",
      "sources": ["ActivityUserInput", "UserInput", "ReceiverUserInput", "ProviderUserInput"],
      "sinks": ["UriGrant", "SetResult"]
    },
    {
      "name": "Sensitive data to clipboard",
      "code": 16,
      "description": "Sensitive or user-controlled data may be written to the clipboard",
      "sources": ["ActivityUserInput", "UserInput", "DeviceId", "Location", "Clipboard"],
      "sinks": ["ClipboardWrite"]
    },
    {
      "name": "PII to network exfil",
      "code": 18,
      "description": "Device identifiers or location may flow into network APIs (destination may be unresolved; crypto does not clear taint)",
      "sources": ["DeviceId", "Location", "Clipboard"],
      "sinks": ["Network", "CookieWrite", "Logging"]
    },
    {
      "name": "SSL / hostname verification bypass surface",
      "code": 17,
      "description": "Untrusted values or custom trust managers may weaken TLS hostname/certificate checks",
      "sources": ["ActivityUserInput", "UserInput", "UntrustedCodePath"],
      "sinks": ["SslBypass"]
    },
    {
      "name": "Nested Intent to component launch (Q3/N1)",
      "code": 19,
      "description": "Parcelable/nested Intent extras may flow into startActivity/startService/bindService/sendBroadcast",
      "sources": ["NestedIntent", "ActivityUserInput", "ReceiverUserInput"],
      "sinks": ["LaunchingComponent", "SetResult", "UriGrant"]
    },
    {
      "name": "Deeplink / extras to CookieManager (Q3/N1)",
      "code": 20,
      "description": "Intent/deeplink-controlled values may reach CookieManager get/setCookie",
      "sources": ["ActivityUserInput", "NestedIntent", "UserInput"],
      "sinks": ["CookieWrite", "CookieRead", "ExecuteJavascript"]
    },
    {
      "name": "Deeplink / extras to Custom Tabs (Q3/N1)",
      "code": 21,
      "description": "Intent-derived URLs may reach CustomTabsIntent.launchUrl",
      "sources": ["ActivityUserInput", "NestedIntent", "UserInput"],
      "sinks": ["CustomTabsLaunch", "ExecuteJavascript", "Network"]
    },
    {
      "name": "Receiver extras to launch / WebView / Cookie",
      "code": 22,
      "description": "BroadcastReceiver-controlled values may reach launch, WebView, or cookie sinks",
      "sources": ["ReceiverUserInput", "NestedIntent"],
      "sinks": ["LaunchingComponent", "ExecuteJavascript", "CookieWrite", "CookieRead", "UriGrant"]
    }
  ]
}
"#;
