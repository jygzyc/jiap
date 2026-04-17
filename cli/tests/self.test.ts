import { jest } from "@jest/globals";
import { buildCliUpdateArgs, executeSelfInstall, getCliPackageMetadata } from "../src/commands/self.js";

describe("self command metadata", () => {
  it("prefers npm env metadata when available", () => {
    expect(getCliPackageMetadata({
      npm_package_name: "@custom/decx-cli",
      npm_package_version: "9.9.9",
    } as NodeJS.ProcessEnv)).toEqual({
      name: "@custom/decx-cli",
      version: "9.9.9",
    });
  });

  it("falls back to package.json metadata when npm env is missing", () => {
    const { name, version } = getCliPackageMetadata({} as NodeJS.ProcessEnv);
    expect(name).toBe("@jygzyc/decx-cli");
    expect(version).toMatch(/^\d+\.\d+\.\d+/);
  });

  it("builds npm install args from the package name", () => {
    expect(buildCliUpdateArgs("@custom/decx-cli")).toEqual([
      "install",
      "-g",
      "@custom/decx-cli@latest",
    ]);
  });

  it("updates stored server version and returns a real install path on self install", async () => {
    const updateServerVersion = jest.fn();
    const result = await executeSelfInstall(false, {
      installDecxServerFn: async () => ({
        ok: true,
        version: "0.0.0",
        path: "/tmp/decx-server.jar",
        message: "Installed decx-server v0.0.0 to /tmp/decx-server.jar",
      }),
      manager: { updateServerVersion },
    });

    expect(updateServerVersion).toHaveBeenCalledWith("0.0.0");
    expect(result).toEqual({
      ok: true,
      version: "0.0.0",
      path: "/tmp/decx-server.jar",
      message: "Installed decx-server v0.0.0 to /tmp/decx-server.jar",
    });
  });
});
