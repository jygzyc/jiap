/**
 * DECX API integration tests (mock-based).
 *
 * Validates every DecxClient API endpoint's response structure against
 * the known server response format. Uses mock fetch — no real server needed.
 *
 * Mock data sourced from real Sieve APK (com.withsecure.example.sieve) via decx-server.
 */

import { DecxClient } from "../src/core/client.js";

let mockFetchImpl: (url: string | URL | Request) => Promise<Response>;
function mockFetch(url: string | URL | Request): Promise<Response> {
    return mockFetchImpl(url);
}

// ── Mock fetch helpers ───────────────────────────────────────────────────

type MockMapping = { path: string; body: unknown };

const mappings: MockMapping[] = [];

function mockResponse(path: string, body: unknown) {
    mappings.push({ path, body });
}

function setupMockFetch() {
    mockFetchImpl = async (url: string | URL | Request) => {
        const urlStr = typeof url === "string" ? url : url.toString();
        const match = mappings.find((m) => urlStr.includes(m.path));
        if (match) {
            return {
                ok: true,
                status: 200,
                json: async () => match.body,
            } as Response;
        }
        return {
            ok: false,
            status: 404,
            json: async () => ({ error: "not found" }),
        } as Response;
    };
}

function teardownMockFetch() {
    mockFetchImpl = async () => ({ ok: false, status: 404, json: async () => ({}) } as Response);
    mappings.length = 0;
}

// ── Shared assertion helpers ─────────────────────────────────────────────

/** Assert a standard { success, data } wrapper. */
function expectSuccessEnvelope(res: Record<string, unknown>) {
    expect(res).toHaveProperty("success", true);
    expect(res).toHaveProperty("data");
    expect(typeof res.data).toBe("object");
}

/** Assert a list-type response: { success, data: { type: "list", count, <listField>: [] } }. */
function expectListResponse(
    res: Record<string, unknown>,
    listField: string,
) {
    expectSuccessEnvelope(res);
    const data = res.data as Record<string, unknown>;
    expect(data.type).toBe("list");
    expect(typeof data.count).toBe("number");
    expect(Array.isArray(data[listField])).toBe(true);
}

/** Assert a code-type response: { success, data: { type: "code", name, code } }. */
function expectCodeResponse(res: Record<string, unknown>) {
    expectSuccessEnvelope(res);
    const data = res.data as Record<string, unknown>;
    expect(data.type).toBe("code");
    expect(typeof data.name).toBe("string");
    expect(typeof data.code).toBe("string");
}

/** Assert a xref-type response: { success, data: { type: "list", count, "references-list": { ... } } }. */
function expectXrefResponse(res: Record<string, unknown>) {
    expectSuccessEnvelope(res);
    const data = res.data as Record<string, unknown>;
    expect(data.type).toBe("list");
    expect(typeof data.count).toBe("number");
    const refs = data["references-list"];
    expect(typeof refs).toBe("object");
    expect(refs).not.toBeNull();
    // Each entry should have fullName, className, codeLineNumber, codeLine
    for (const key of Object.keys(refs as Record<string, unknown>)) {
        const entry = (refs as Record<string, Record<string, unknown>>)[key];
        expect(entry).toHaveProperty("fullName");
        expect(entry).toHaveProperty("className");
        expect(entry).toHaveProperty("codeLineNumber");
        expect(entry).toHaveProperty("codeLine");
    }
}

// ── Test suite ───────────────────────────────────────────────────────────

describe("DECX API integration (Sieve APK)", () => {
    let client: DecxClient;

    beforeAll(() => {
        client = new DecxClient("127.0.0.1", 25419, 10, mockFetch);
    });

    afterAll(() => {
        teardownMockFetch();
    });

    // ── Health ─────────────────────────────────────────────────────────

    describe("health", () => {
        beforeAll(() => {
            mockResponse("/health", {
                status: "running",
                url: "http://198.18.0.1:25419",
                port: 25419,
                timestamp: 1775567879734,
            });
            setupMockFetch();
        });

        it("healthCheck returns { status: 'running' }", async () => {
            const res = await client.healthCheck();
            expect(res).toHaveProperty("status", "running");
        });

        it("isHealthy returns true", async () => {
            const res = await client.isHealthy();
            expect(res).toBe(true);
        });
    });

    // ── CommonService ──────────────────────────────────────────────────

    describe("CommonService", () => {
        // getAllClasses
        describe("getAllClasses", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_all_classes", {
                    success: true,
                    data: {
                        type: "list",
                        count: 6588,
                        "classes-list": [
                            "android.support.v4.app.INotificationSideChannel",
                            "android.support.v4.app.INotificationSideChannel.Default",
                            "android.support.v4.app.INotificationSideChannel.Stub",
                            "android.support.v4.app.INotificationSideChannel.Stub.Proxy",
                            "android.support.v4.app.INotificationSideChannel._Parcel",
                        ],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with classes-list array", async () => {
                const res = await client.getAllClasses();
                expectListResponse(res, "classes-list");
            });
        });

        // getClassInfo
        describe("getClassInfo", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_class_info", {
                    name: "com.withsecure.example.sieve.activity.WelcomeActivity",
                    type: "list",
                    "fields-list": [
                        "com.withsecure.example.sieve.activity.WelcomeActivity.IS_AUTHENTICATED :int",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.MAIN_PIN :int",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.MAIN_SETTINGS :int",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.MAIN_WELCOME :int",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.NOT_AUTHENTICATED :int",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.NOT_INITALISED :int",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.TAG :java.lang.String",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.entry :android.widget.EditText",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.login_button :android.widget.Button",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.prompt :android.widget.TextView",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.serviceConnection :com.withsecure.example.sieve.service.AuthServiceConnector",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.state :int",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.workingPassword :java.lang.String",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.workingIntent :android.content.Intent",
                    ],
                    "methods-list": [
                        "com.withsecure.example.sieve.activity.WelcomeActivity.<init>():void",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.checkKeyResult(boolean):void",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.checkPinResult(boolean):void",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.connected():void",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.firstLaunchResult(int):void",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.initaliseActivity():void",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.lambda$sendFailed$0(android.content.DialogInterface, int):void",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.login(android.view.View):void",
                        "com.withsecure.example.sieve.activity.WelcomeActivity.loginFailed():void",
                    ],
                });
                setupMockFetch();
            });

            it("returns list-type response with name, methods-list, and fields-list", async () => {
                const res = await client.getClassInfo("com.withsecure.example.sieve.activity.WelcomeActivity");
                // getClassInfo returns flat response (no success/data wrapper)
                expect(res.type).toBe("list");
                expect(typeof res.name).toBe("string");
                expect(Array.isArray(res["methods-list"])).toBe(true);
                expect(Array.isArray(res["fields-list"])).toBe(true);
            });
        });

        // getClassSource
        describe("getClassSource", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_class_source", {
                    name: "com.withsecure.example.sieve.activity.WelcomeActivity",
                    type: "code",
                    code: "package com.withsecure.example.sieve.activity;\n\nimport android.app.Activity;\nimport android.app.AlertDialog;\nimport android.content.DialogInterface;\nimport android.content.Intent;\nimport android.os.Bundle;\nimport android.util.Log;\nimport android.view.View;\nimport android.widget.Button;\nimport android.widget.EditText;\nimport android.widget.TextView;\nimport com.withsecure.example.sieve.R;\nimport com.withsecure.example.sieve.service.AuthService;\nimport com.withsecure.example.sieve.service.AuthServiceConnector;\nimport com.withsecure.example.sieve.service.CryptoService;\n\n/* JADX INFO: loaded from: classes.dex */\npublic class WelcomeActivity extends Activity implements AuthServiceConnector.ResponseListener {\n    private static final int IS_AUTHENTICATED = 4521387;\n    public static final int MAIN_PIN = 2;\n    public static final int MAIN_SETTINGS = 3;\n    public static final int MAIN_WELCOME = 1;\n    private static final int NOT_AUTHENTICATED = 654987;\n    private static final int NOT_INITALISED = 923472;\n    private static final String TAG = \"m_MainLogin\";\n    EditText entry;\n    Button login_button;\n    TextView prompt;\n    private AuthServiceConnector serviceConnection;\n    private int state = NOT_INITALISED;\n    private String workingPassword = null;\n    private Intent workingIntent = null;\n\n    @Override\n    public void checkKeyResult(boolean status) {\n        if (status) { loginSuccessful(); } else { loginFailed(); }\n    }\n\n    public void login(View view) {\n        this.workingPassword = this.entry.getText().toString();\n        Log.d(TAG, \"String enetered: \" + this.workingPassword);\n        this.serviceConnection.checkKey(this.workingPassword);\n        this.login_button.setEnabled(false);\n    }\n}",
                });
                setupMockFetch();
            });

            it("returns code-type response with name and code", async () => {
                const res = await client.getClassSource("com.withsecure.example.sieve.activity.WelcomeActivity");
                // getClassSource returns flat response (no success/data wrapper)
                expect(res.type).toBe("code");
                expect(typeof res.name).toBe("string");
                expect(typeof res.code).toBe("string");
            });
        });

        // getMethodSource
        describe("getMethodSource", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_method_source", {
                    name: "com.withsecure.example.sieve.activity.WelcomeActivity.login(android.view.View):void",
                    type: "code",
                    code: "    public void login(View view) {\n        this.workingPassword = this.entry.getText().toString();\n        Log.d(TAG, \"String enetered: \" + this.workingPassword);\n        this.serviceConnection.checkKey(this.workingPassword);\n        this.login_button.setEnabled(false);\n    }",
                });
                setupMockFetch();
            });

            it("returns code-type response with name and code", async () => {
                const res = await client.getMethodSource(
                    "com.withsecure.example.sieve.activity.WelcomeActivity.login(android.view.View):void",
                );
                // getMethodSource returns flat response (no success/data wrapper)
                expect(res.type).toBe("code");
                expect(typeof res.name).toBe("string");
                expect(typeof res.code).toBe("string");
            });
        });

        // searchClassKey
        describe("searchClassKey", () => {
            beforeAll(() => {
                mockResponse("/api/decx/search_class_key", {
                    success: true,
                    data: {
                        type: "list",
                        count: 8,
                        "classes-list": [
                            "androidx.core.hardware.fingerprint.FingerprintManagerCompat",
                            "com.withsecure.example.sieve.activity.WelcomeActivity",
                            "com.withsecure.example.sieve.activity.PWList",
                            "com.withsecure.example.sieve.activity.SettingsActivity",
                            "com.withsecure.example.sieve.activity.ShortLoginActivity",
                            "com.withsecure.example.sieve.service.AuthServiceConnector",
                            "com.withsecure.example.sieve.service.CryptoService",
                            "com.withsecure.example.sieve.service.CryptoServiceConnector",
                        ],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with classes-list array", async () => {
                const res = await client.searchClassKey("Crypto");
                expectListResponse(res, "classes-list");
            });
        });

        // searchMethod
        describe("searchMethod", () => {
            beforeAll(() => {
                mockResponse("/api/decx/search_method", {
                    success: true,
                    data: {
                        type: "list",
                        count: 6,
                        "methods-list": [
                            "com.withsecure.example.sieve.activity.PWList.encryptionReturned(byte[], int):void",
                            "com.withsecure.example.sieve.activity.SettingsActivity.encryptionReturned(byte[], int):void",
                            "com.withsecure.example.sieve.service.CryptoService.encrypt(java.lang.String, java.lang.String):byte[]",
                            "com.withsecure.example.sieve.service.CryptoService.runNDKencrypt(java.lang.String, java.lang.String):byte[]",
                            "com.withsecure.example.sieve.service.CryptoServiceConnector.sendForEncryption(java.lang.String, java.lang.String, int):void",
                            "com.withsecure.example.sieve.service.CryptoServiceConnector.ResponseListener.encryptionReturned(byte[], int):void",
                        ],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with methods-list array", async () => {
                const res = await client.searchMethod("encrypt");
                expectListResponse(res, "methods-list");
            });
        });

        // getMethodXref
        describe("getMethodXref", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_method_xref", {
                    success: true,
                    data: {
                        type: "list",
                        count: 2,
                        "references-list": {
                            "154031532743": {
                                fullName: "com.withsecure.example.sieve.service.CryptoService.MessageHandler.handleMessage",
                                codeLine: "recievedBundle.putByteArray(CryptoService.RESULT, CryptoService.this.encrypt(recievedKey, recievedString));",
                                className: "com.withsecure.example.sieve.service.CryptoService",
                                codeLineNumber: 43,
                            },
                        },
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with references-list entries", async () => {
                const res = await client.getMethodXref(
                    "com.withsecure.example.sieve.service.CryptoService.encrypt(java.lang.String, java.lang.String):byte[]",
                );
                expectXrefResponse(res);
            });
        });

        // getFieldXref
        describe("getFieldXref", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_field_xref", {
                    success: true,
                    data: {
                        type: "list",
                        count: 1,
                        "references-list": {
                            "0": {
                                fullName: "com.withsecure.example.sieve.activity.WelcomeActivity.login(android.view.View):void",
                                className: "com.withsecure.example.sieve.activity.WelcomeActivity",
                                codeLineNumber: 10,
                                codeLine: "this.workingPassword = this.entry.getText().toString();",
                            },
                        },
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with references-list entries", async () => {
                const res = await client.getFieldXref("com.withsecure.example.sieve.activity.WelcomeActivity.entry :android.widget.EditText");
                expectXrefResponse(res);
            });
        });

        // getClassXref
        describe("getClassXref", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_class_xref", {
                    success: true,
                    data: {
                        type: "list",
                        count: 13,
                        "references-list": {
                            "-35680001870": {
                                fullName: "com.withsecure.example.sieve.service.CryptoService.MessageHandler",
                                codeLine: "Log.e(CryptoService.TAG, \"Unable to send message: \" + command);",
                                className: "com.withsecure.example.sieve.service.CryptoService",
                                codeLineNumber: 70,
                            },
                            "127465132236": {
                                fullName: "com.withsecure.example.sieve.activity.PWList",
                                codeLine: "bindService(new Intent(this, (Class<?>) CryptoService.class), this.serviceConnection, 1);",
                                className: "com.withsecure.example.sieve.activity.PWList",
                                codeLineNumber: 236,
                            },
                        },
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with references-list entries", async () => {
                const res = await client.getClassXref("com.withsecure.example.sieve.service.CryptoService");
                expectXrefResponse(res);
            });
        });

        // getImplement
        describe("getImplement", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_implement", {
                    success: true,
                    data: {
                        type: "list",
                        count: 1,
                        "classes-list": ["com.withsecure.example.sieve.provider.DBContentProvider"],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with classes-list array", async () => {
                const res = await client.getImplement("android.content.ContentProvider");
                expectListResponse(res, "classes-list");
            });
        });

        // getSubClasses
        describe("getSubClasses", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_sub_classes", {
                    success: true,
                    data: {
                        type: "list",
                        count: 2,
                        "classes-list": [
                            "com.withsecure.example.sieve.activity.WelcomeActivity",
                            "com.withsecure.example.sieve.activity.FileSelectActivity",
                        ],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with classes-list array", async () => {
                const res = await client.getSubClasses("android.app.Activity");
                expectListResponse(res, "classes-list");
            });
        });
    });

    // ── AndroidAppService ──────────────────────────────────────────────

    describe("AndroidAppService", () => {
        // getAppManifest
        describe("getAppManifest", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_app_manifest", {
                    success: true,
                    data: {
                        type: "code",
                        name: "AndroidManifest.xml",
                        code: '<?xml version="1.0" encoding="utf-8"?>\n<manifest xmlns:android="http://schemas.android.com/apk/res/android"\n    package="com.withsecure.example.sieve">\n</manifest>',
                    },
                });
                setupMockFetch();
            });

            it("returns code-type response with AndroidManifest.xml", async () => {
                const res = await client.getAppManifest();
                expectCodeResponse(res);
            });
        });

        // getMainActivity
        describe("getMainActivity", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_main_activity", {
                    name: "com.withsecure.example.sieve.activity.WelcomeActivity",
                    type: "code",
                    code: "package com.withsecure.example.sieve.activity;\n\nimport android.app.Activity;\nimport android.app.AlertDialog;\nimport android.content.DialogInterface;\nimport android.content.Intent;\nimport android.os.Bundle;\nimport android.util.Log;\nimport android.view.View;\nimport android.widget.Button;\nimport android.widget.EditText;\nimport android.widget.TextView;\nimport com.withsecure.example.sieve.R;\nimport com.withsecure.example.sieve.service.AuthService;\nimport com.withsecure.example.sieve.service.AuthServiceConnector;\nimport com.withsecure.example.sieve.service.CryptoService;\n\n/* JADX INFO: loaded from: classes.dex */\npublic class WelcomeActivity extends Activity implements AuthServiceConnector.ResponseListener {\n    private static final int IS_AUTHENTICATED = 4521387;\n    public static final int MAIN_PIN = 2;\n    public static final int MAIN_SETTINGS = 3;\n    public static final int MAIN_WELCOME = 1;\n    private static final int NOT_AUTHENTICATED = 654987;\n    private static final int NOT_INITALISED = 923472;\n    private static final String TAG = \"m_MainLogin\";\n    EditText entry;\n    Button login_button;\n    TextView prompt;\n    private AuthServiceConnector serviceConnection;\n    private int state = NOT_INITALISED;\n    private String workingPassword = null;\n    private Intent workingIntent = null;\n\n    @Override\n    public void checkKeyResult(boolean status) {\n        if (status) { loginSuccessful(); } else { loginFailed(); }\n    }\n\n    public void login(View view) {\n        this.workingPassword = this.entry.getText().toString();\n        Log.d(TAG, \"String enetered: \" + this.workingPassword);\n        this.serviceConnection.checkKey(this.workingPassword);\n        this.login_button.setEnabled(false);\n    }\n}",
                });
                setupMockFetch();
            });

            it("returns code-type response with main activity source", async () => {
                const res = await client.getMainActivity();
                // getMainActivity returns flat response (no success/data wrapper)
                expect(res.type).toBe("code");
                expect(typeof res.name).toBe("string");
                expect(typeof res.code).toBe("string");
            });
        });

        // getApplication
        describe("getApplication", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_application", {
                    error: "handleGetApplication: no Application class found",
                });
                setupMockFetch();
            });

            it("returns error when no Application class found", async () => {
                const res = await client.getApplication();
                expect(res).toHaveProperty("error");
            });
        });

        // getExportedComponents
        describe("getExportedComponents", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_exported_components", {
                    success: true,
                    data: {
                        type: "list",
                        count: 8,
                        "components-list": [
                            {
                                name: "com.withsecure.example.sieve.activity.WelcomeActivity",
                                launchMode: "singleTask",
                                intentFilters: [
                                    {
                                        categories: ["android.intent.category.LAUNCHER"],
                                        actions: ["android.intent.action.MAIN"],
                                    },
                                ],
                                type: "activity",
                            },
                            {
                                name: "com.withsecure.example.sieve.activity.FileSelectActivity",
                                type: "activity",
                            },
                            {
                                name: "com.withsecure.example.sieve.activity.PWList",
                                type: "activity",
                            },
                            {
                                name: "com.withsecure.example.sieve.service.AuthService",
                                type: "service",
                            },
                            {
                                name: "com.withsecure.example.sieve.service.CryptoService",
                                type: "service",
                            },
                            {
                                name: "androidx.profileinstaller.ProfileInstallReceiver",
                                intentFilters: [
                                    { actions: ["androidx.profileinstaller.action.INSTALL_PROFILE"] },
                                    { actions: ["androidx.profileinstaller.action.SKIP_FILE"] },
                                    { actions: ["androidx.profileinstaller.action.SAVE_PROFILE"] },
                                    { actions: ["androidx.profileinstaller.action.BENCHMARK_OPERATION"] },
                                ],
                                permission: "android.permission.DUMP",
                                type: "receiver",
                            },
                            {
                                name: "com.withsecure.example.sieve.provider.DBContentProvider",
                                type: "provider",
                                authorities: "com.withsecure.example.sieve.provider.DBContentProvider",
                            },
                            {
                                name: "com.withsecure.example.sieve.provider.FileBackupProvider",
                                type: "provider",
                                authorities: "com.withsecure.example.sieve.provider.FileBackupProvider",
                            },
                        ],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with components-list array", async () => {
                const res = await client.getExportedComponents();
                expectListResponse(res, "components-list");
            });

            it("each component has name and type", async () => {
                const res = await client.getExportedComponents();
                const data = res.data as Record<string, unknown>;
                const components = data["components-list"] as Record<string, unknown>[];
                for (const comp of components) {
                    expect(typeof comp.name).toBe("string");
                    expect(typeof comp.type).toBe("string");
                }
            });
        });

        // getDeepLinks
        describe("getDeepLinks", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_deep_links", {
                    success: true,
                    data: {
                        type: "list",
                        count: 0,
                        "deeplinks-list": [],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with empty deeplinks-list", async () => {
                const res = await client.getDeepLinks();
                expectListResponse(res, "deeplinks-list");
                const data = res.data as Record<string, unknown>;
                expect((data["deeplinks-list"] as unknown[]).length).toBe(0);
            });
        });

        // getDynamicReceivers
        describe("getDynamicReceivers", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_dynamic_receivers", {
                    success: true,
                    data: {
                        type: "list",
                        count: 10,
                        "code-list": [
                            {
                                method: "        void cleanup() {\n            if (this.mReceiver != null) {\n                try {\n                    AppCompatDelegateImpl.this.mContext.unregisterReceiver(this.mReceiver);\n                } catch (IllegalArgumentException e) {\n                }\n                this.mReceiver = null;\n            }\n        }",
                                class: "androidx.appcompat.app.AppCompatDelegateImpl.AutoNightModeManager",
                            },
                            {
                                method: "    public static Intent registerReceiver(Context context, BroadcastReceiver receiver, IntentFilter filter, int flags) {\n        return registerReceiver(context, receiver, filter, null, null, flags);\n    }",
                                class: "androidx.core.content.ContextCompat",
                            },
                        ],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with code-list array", async () => {
                const res = await client.getDynamicReceivers();
                expectListResponse(res, "code-list");
            });

            it("each entry has class and method", async () => {
                const res = await client.getDynamicReceivers();
                const data = res.data as Record<string, unknown>;
                const entries = data["code-list"] as Record<string, unknown>[];
                for (const entry of entries) {
                    expect(typeof entry.class).toBe("string");
                    expect(typeof entry.method).toBe("string");
                }
            });
        });

        // getAllResources
        describe("getAllResources", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_all_resources", {
                    success: true,
                    data: {
                        type: "list",
                        count: 1174,
                        "resources-list": [
                            "lib/x86_64/libsieve.so",
                            "lib/arm64-v8a/libc++_shared.so",
                            "lib/x86_64/libc++_shared.so",
                            "lib/armeabi-v7a/libc++_shared.so",
                            "META-INF/com/android/build/gradle/app-metadata.properties",
                        ],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with resources-list array", async () => {
                const res = await client.getAllResources();
                expectListResponse(res, "resources-list");
            });
        });

        // getResourceFile
        describe("getResourceFile", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_resource_file", {
                    name: "res/layout/activity_main_login.xml",
                    type: "code",
                    code: "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<RelativeLayout xmlns:android=\"http://schemas.android.com/apk/res/android\"\n    android:background=\"@color/background\"\n    android:layout_width=\"match_parent\"\n    android:layout_height=\"match_parent\">\n    <Button\n        android:id=\"@+id/mainlogin_button_login\"\n        android:paddingLeft=\"32dp\"\n        android:paddingRight=\"32dp\"\n        android:layout_width=\"wrap_content\"\n        android:layout_height=\"wrap_content\"\n        android:layout_marginLeft=\"40dp\" />",
                });
                setupMockFetch();
            });

            it("returns code-type response with resource file content", async () => {
                const res = await client.getResourceFile("res/layout/activity_main_login.xml");
                // getResourceFile returns flat response (no success/data wrapper)
                expect(res.type).toBe("code");
                expect(typeof res.name).toBe("string");
                expect(typeof res.code).toBe("string");
            });
        });

        // getStrings
        describe("getStrings", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_strings", {
                    success: true,
                    data: {
                        type: "list",
                        count: 1,
                        "strings-list": [
                            {
                                file: "res/values/strings.xml",
                                content: '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n    <string name="app_name">Sieve</string>\n</resources>',
                            },
                        ],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with strings-list array", async () => {
                const res = await client.getStrings();
                expectListResponse(res, "strings-list");
            });

            it("each string entry has file and content", async () => {
                const res = await client.getStrings();
                const data = res.data as Record<string, unknown>;
                const strings = data["strings-list"] as Record<string, unknown>[];
                for (const s of strings) {
                    expect(typeof s.file).toBe("string");
                    expect(typeof s.content).toBe("string");
                }
            });
        });
    });

    // ── AndroidFrameworkService ────────────────────────────────────────

    describe("AndroidFrameworkService", () => {
        describe("getSystemServiceImpl", () => {
            beforeAll(() => {
                mockResponse("/api/decx/get_system_service_impl", {
                    success: true,
                    data: {
                        type: "list",
                        count: 1,
                        "classes-list": ["com.withsecure.example.sieve.service.CryptoService"],
                    },
                });
                setupMockFetch();
            });

            it("returns list-type response with classes-list array", async () => {
                const res = await client.getSystemServiceImpl("android.content.ServiceConnection");
                expectListResponse(res, "classes-list");
            });
        });
    });
});
