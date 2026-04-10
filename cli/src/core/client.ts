/**
 * JIAP HTTP Client - Direct HTTP client for JIAP Server.
 *
 * Communicates with the JIAP server via REST API endpoints.
 * All methods return raw JSON responses — no data unwrapping.
 */

import { JiapError } from "../utils/errors.js";

export { JiapError as JIAPError };

export type FetchFn = typeof globalThis.fetch;

export class JIAPClient {
    private baseUrl: string;
    private timeout: number;
    private _fetch: FetchFn;

    constructor(host: string = "127.0.0.1", port: number = 25419, timeout: number = 30, fetchFn?: FetchFn) {
        this.baseUrl = `http://${host}:${port}`;
        this.timeout = timeout * 1000;
        this._fetch = fetchFn ?? globalThis.fetch;
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

        // Debug log (opt-in via JIAP_DEBUG=1)
        if (process.env.JIAP_DEBUG === "1") {
            console.error(`[DEBUG] ${method} ${url}`);
            if (data) console.error(`[DEBUG] Body: ${JSON.stringify(data)}`);
        }

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
                return (await response.json()) as T;
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
            throw new JiapError(errorMessage, errorCode);
        } catch (err) {
            if (err instanceof JiapError) throw err;
            if ((err as Error).name === "AbortError") {
                throw new JiapError("Request timed out", "TIMEOUT");
            }
            throw new JiapError(`Connection failed: ${(err as Error).message}`, "CONNECTION_ERROR");
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
        return this.request("POST", "/api/jiap/get_all_classes", { page });
    }

    async getClassInfo(cls: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_class_info", { cls, page });
    }

    async getClassSource(cls: string, smali: boolean = false, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_class_source", { cls, smali, page });
    }

    async getMethodSource(mth: string, smali: boolean = false, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_method_source", { mth, smali, page });
    }

    async searchClassKey(key: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/search_class_key", { key, page });
    }

    async searchMethod(mth: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/search_method", { mth, page });
    }

    async getMethodXref(mth: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_method_xref", { mth, page });
    }

    async getFieldXref(fld: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_field_xref", { fld, page });
    }

    async getClassXref(cls: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_class_xref", { cls, page });
    }

    async getImplement(iface: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_implement", { iface, page });
    }

    async getSubClasses(cls: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_sub_classes", { cls, page });
    }

    // ── AndroidAppService ───────────────────────────────────────────────────

    async getAppManifest(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_app_manifest", { page });
    }

    async getMainActivity(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_main_activity", { page });
    }

    async getApplication(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_application", { page });
    }

    async getExportedComponents(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_exported_components", { page });
    }

    async getDeepLinks(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_deep_links", { page });
    }

    async getDynamicReceivers(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_dynamic_receivers", { page });
    }

    async getAllResources(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_all_resources", { page });
    }

    async getResourceFile(res: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_resource_file", { res, page });
    }

    async getStrings(page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_strings", { page });
    }

    // ── AndroidFrameworkService ─────────────────────────────────────────────

    async getSystemServiceImpl(iface: string, page: number = 1): Promise<Record<string, unknown>> {
        return this.request("POST", "/api/jiap/get_system_service_impl", { iface, page });
    }

}
