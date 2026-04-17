import { jest } from "@jest/globals";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import * as path from "path";
import {
  installDecxServer,
  selectDecxServerAsset,
  type ReleaseAsset,
} from "../src/server/installer.js";

describe("installer", () => {
  it("selects the jar asset instead of similarly named non-jar assets", () => {
    const assets: ReleaseAsset[] = [
      { name: "decx-server.sha256", browser_download_url: "https://example.invalid/sha" },
      { name: "decx-server-0.0.0.jar", browser_download_url: "https://example.invalid/jar" },
    ];

    expect(selectDecxServerAsset(assets)).toEqual(assets[1]);
  });

  it("overwrites an existing installed jar and returns normalized version/path metadata", async () => {
    const installDir = mkdtempSync(path.join(tmpdir(), "decx-installer-"));
    const installPath = path.join(installDir, "decx-server.jar");
    const logger = { error: jest.fn() };

    writeFileSync(installPath, "old-jar", "utf-8");

    const fetchImpl = jest.fn(async (url: string | URL | Request) => {
      const href = typeof url === "string" ? url : url instanceof URL ? url.href : url.url;
      if (href.includes("/releases/latest")) {
        return new Response(JSON.stringify({
          tag_name: "v2.6.0",
          assets: [
            { name: "decx-server.sha256", browser_download_url: "https://example.invalid/sha" },
            { name: "decx-server-2.6.0.jar", browser_download_url: "https://example.invalid/jar" },
          ],
        }), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      return new Response("jar-bytes", {
        status: 200,
        headers: { "content-length": "9" },
      });
    }) as typeof fetch;

    const downloadWithProgressImpl = jest.fn(async (_body, filePath: string) => {
      writeFileSync(filePath, "new-jar", "utf-8");
      return 7;
    });

    try {
      const result = await installDecxServer(false, {
        installDir,
        installPath,
        fetchImpl,
        downloadWithProgressImpl,
        logger,
      });

      expect(result).toEqual({
        ok: true,
        version: "2.6.0",
        path: installPath,
        message: `Installed decx-server v2.6.0 to ${installPath}`,
      });
      expect(readFileSync(installPath, "utf-8")).toBe("new-jar");
      expect(existsSync(`${installPath}.bak`)).toBe(false);
      expect(fetchImpl).toHaveBeenCalledTimes(2);
    } finally {
      rmSync(installDir, { recursive: true, force: true });
    }
  });
});
