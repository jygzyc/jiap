import { buildCliUpdateArgs, getCliPackageMetadata } from "../src/commands/self.js";

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
    expect(getCliPackageMetadata({} as NodeJS.ProcessEnv)).toEqual({
      name: "@jygzyc/decx-cli",
      version: "2.5.0",
    });
  });

  it("builds npm install args from the package name", () => {
    expect(buildCliUpdateArgs("@custom/decx-cli")).toEqual([
      "install",
      "-g",
      "@custom/decx-cli@latest",
    ]);
  });
});
