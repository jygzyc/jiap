/**
 * DECX HTTP Client - Direct HTTP client for DECX Server.
 *
 * Communicates with the DECX server via REST API endpoints.
 * All methods return raw JSON responses — no data unwrapping.
 */

import { DecxError } from "../utils/errors.js";
import { logApiCall } from "../utils/logger.js";

export { DecxError };

export type FetchFn = typeof globalThis.fetch;

export class DecxClient {
    private baseUrl: string;
    private timeout: number;
    private _fetch: FetchFn;
    private sessionName: string | null;

    constructor(host: string = "127.0.0.1", port: number = 25419, timeout: number = 30, fetchFn?: FetchFn, sessionName?: string) {
        this.baseUrl = `http://${host}:${port}`;
        this.timeout = timeout * 1000;
        this._fetch = fetchFn ?? globalThis.fetch;
        this.sessionName = sessionName ?? null;
    }

    private async request<T = unknown>(
        method: "GET" | "POST",
        apiPath: string,
        data?: Record<string, unknown>
    ): Promise<T> {
        const url = `${this.baseUrl}${apiPath}`;
        const headers: Record<string, string> = {
            Accept: "application/json",
            "Content-Type": "application/json",
        };

        // Debug log (opt-in via DECX_DEBUG=1)
        if (process.env.DECX_DEBUG === "1") {
            console.error(`[DEBUG] ${method} ${url}`);
            if (data) console.error(`[DEBUG] Body: ${JSON.stringify(data)}`);
        }

        const startTime = Date.now();
        let status: "ok" | "error" = "error";
        let errorMsg: string | undefined;

        try {
            const controller = new AbortController();
            const timeoutId = setTimeout(() => controller.abort(), this.timeout);

            const response = await this._fetch(url, {
                method,
                headers,
                body: method === "POST" && data ? JSON.stringify(data) : undefined,
                signal: controller.signal,
            });

            clearTimeout(timeoutId);

            if (response.status === 200) {
                status = "ok";
                return await response.json() as T;
            }

            // Server returns { "error": "E0xx", "message": "..." }
            let errorMessage = `HTTP ${response.status}`;
            let errorCode = `HTTP_${response.status}`;
            try {
                const errorBody = (await response.json()) as { error?: string; message?: string };
                if (errorBody.error) {
                    errorCode = errorBody.error;
                    errorMessage = errorBody.message ?? errorBody.error;
                }
            } catch {
                // ignore parse errors
            }
            errorMsg = errorMessage;
            throw new DecxError(errorMessage, errorCode);
        } catch (err) {
            if (err instanceof DecxError) throw err;
            if ((err as Error).name === "AbortError") {
                throw new DecxError("Request timed out", "TIMEOUT");
            }
            throw new DecxError(`Connection failed: ${(err as Error).message}`, "CONNECTION_ERROR");
        } finally {
            if (this.sessionName) {
                logApiCall(this.sessionName, {
                    method,
                    path: apiPath,
                    duration_ms: Date.now() - startTime,
                    status,
                    error: errorMsg,
                });
            }
        }
    }

    // ── Health Check ────────────────────────────────────────────────────────

    async healthCheck(): Promise<Record<string, unknown>> {
        return this.request("GET", "/health");
    }

    async isHealthy(): Promise<boolean> {
        try {
            const result = await this.healthCheck();
            return result.status === "running";
        } catch {
            return false;
        }
    }

    // ── CommonService ───────────────────────────────────────────────────────

    async getAllClasses(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_all_classes", { page });
    }

    async getClassInfo(cls: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_class_info", { cls, page });
    }

    async getClassSource(cls: string, smali: boolean = false, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_class_source", { cls, smali, page });
    }

    async getMethodSource(mth: string, smali: boolean = false, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_method_source", { mth, smali, page });
    }

    async searchClassKey(key: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/search_class_key", { key, page });
    }

    async searchMethod(mth: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/search_method", { mth, page });
    }

    async getMethodXref(mth: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_method_xref", { mth, page });
    }

    async getFieldXref(fld: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_field_xref", { fld, page });
    }

    async getClassXref(cls: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_class_xref", { cls, page });
    }

    async getImplement(iface: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_implement", { iface, page });
    }

    async getSubClasses(cls: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_sub_classes", { cls, page });
    }

    // ── Android App Service ───────────────────────────────────────────────────

    async getAidlInterfaces(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_aidl", { page });
    }

    async getAppManifest(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_app_manifest", { page });
    }

    async getMainActivity(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_main_activity", { page });
    }

    async getApplication(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_application", { page });
    }

    async getExportedComponents(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_exported_components", { page });
    }

    async getDeepLinks(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_deep_links", { page });
    }

    async getDynamicReceivers(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_dynamic_receivers", { page });
    }

    async getAllResources(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_all_resources", { page });
    }

    async getResourceFile(res: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_resource_file", { res, page });
    }

    async getStrings(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_strings", { page });
    }

    // ── AndroidFrameworkService ─────────────────────────────────────────────

    async getSystemServiceImpl(iface: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/decx/get_system_service_impl", { iface, page });
    }

}
