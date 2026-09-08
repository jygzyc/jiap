/**
 * Tiered framework collection tests: ready-made files (framework dirs + /apex
 * mount) first, .apex/.capex images from /system/apex only for modules the
 * mount did not already cover.
 */
import { mkdirSync, rmSync, writeFileSync } from "fs";
import * as path from "path";
import { collectFrameworkFiles } from "../src/android/framework-collector.js";
import type { AdbClient } from "../src/android/adb.js";
import { resetTestDir } from "./test-paths.js";

interface FakeAdbPlan {
  primary: string[];
  images: string[];
  failFor?: (remote: string) => boolean;
}

function fakeAdb(plan: FakeAdbPlan, pulls: string[]): AdbClient {
  return {
    shell: (command: string) => (command.includes("/system/apex") ? plan.images.join("\n") : plan.primary.join("\n")),
    pull: (remote: string) => {
      if (plan.failFor?.(remote)) throw new Error("pull failed");
      pulls.push(remote);
    },
  } as unknown as AdbClient;
}

describe("framework tiered collection", () => {
  it("pulls /apex jars and skips .apex images of covered modules", async () => {
    const rootDir = resetTestDir("tmp", "fw-collect-tiered");
    const sourceDir = path.join(rootDir, "source");
    const pulls: string[] = [];
    const adb = fakeAdb(
      {
        primary: ["/system/framework/framework.jar", "/apex/com.android.art/javalib/core-oj.jar"],
        images: ["/system/apex/com.android.art.apex", "/system/apex/com.android.os.statsd.apex"],
      },
      pulls,
    );

    const result = await collectFrameworkFiles(adb, "google", sourceDir);

    expect(pulls).toEqual([
      "/system/framework/framework.jar",
      "/apex/com.android.art/javalib/core-oj.jar",
      "/system/apex/com.android.os.statsd.apex",
    ]);
    expect(result.skippedCoveredModules).toBe(1);
    expect(result.pulled).toBe(3);
    expect(result.failed).toBe(0);
    expect(result.scanned).toBe(4);
    expect(result.files).toContain(
      path.join(sourceDir, "apex", "com.android.art", "javalib", "core-oj.jar"),
    );

    rmSync(rootDir, { recursive: true, force: true });
  });

  it("treats versioned /apex module dirs as covering their .apex image", async () => {
    const rootDir = resetTestDir("tmp", "fw-collect-versioned");
    const sourceDir = path.join(rootDir, "source");
    const pulls: string[] = [];
    const adb = fakeAdb(
      {
        primary: ["/apex/com.android.art@341100000/javalib/core-oj.jar"],
        images: ["/system/apex/com.android.art.apex"],
      },
      pulls,
    );

    const result = await collectFrameworkFiles(adb, "xiaomi", sourceDir);

    expect(pulls).toEqual(["/apex/com.android.art@341100000/javalib/core-oj.jar"]);
    expect(result.skippedCoveredModules).toBe(1);

    rmSync(rootDir, { recursive: true, force: true });
  });

  it("falls back to .apex images for every module when /apex has nothing", async () => {
    const rootDir = resetTestDir("tmp", "fw-collect-fallback");
    const sourceDir = path.join(rootDir, "source");
    const pulls: string[] = [];
    const adb = fakeAdb(
      {
        primary: ["/system/framework/framework.jar"],
        images: ["/system/apex/com.android.art.apex", "/system/apex/com.android.i18n.capex"],
      },
      pulls,
    );

    const result = await collectFrameworkFiles(adb, "oppo", sourceDir);

    expect(pulls).toEqual([
      "/system/framework/framework.jar",
      "/system/apex/com.android.art.apex",
      "/system/apex/com.android.i18n.capex",
    ]);
    expect(result.skippedCoveredModules).toBe(0);
    expect(result.pulled).toBe(3);

    rmSync(rootDir, { recursive: true, force: true });
  });

  it("skips .apex images already collected by a previous run", async () => {
    const rootDir = resetTestDir("tmp", "fw-collect-resume");
    const sourceDir = path.join(rootDir, "source");
    mkdirSync(path.join(sourceDir, "apex", "com.android.i18n", "javalib"), { recursive: true });
    writeFileSync(path.join(sourceDir, "apex", "com.android.i18n", "javalib", "i18n.jar"), "jar");
    const pulls: string[] = [];
    const adb = fakeAdb(
      { primary: [], images: ["/system/apex/com.android.i18n.apex"] },
      pulls,
    );

    const result = await collectFrameworkFiles(adb, "google", sourceDir);

    expect(pulls).toEqual([]);
    expect(result.skippedCoveredModules).toBe(1);

    rmSync(rootDir, { recursive: true, force: true });
  });

  it("records pull failures without aborting the run", async () => {
    const rootDir = resetTestDir("tmp", "fw-collect-failure");
    const sourceDir = path.join(rootDir, "source");
    const pulls: string[] = [];
    const adb = fakeAdb(
      {
        primary: ["/apex/com.android.art/javalib/core-oj.jar"],
        images: ["/system/apex/com.android.os.statsd.apex"],
        failFor: (remote) => remote.includes("com.android.art"),
      },
      pulls,
    );

    const result = await collectFrameworkFiles(adb, "google", sourceDir);

    expect(pulls).toEqual(["/system/apex/com.android.os.statsd.apex"]);
    expect(result.failed).toBe(1);
    expect(result.failures[0].path).toBe("/apex/com.android.art/javalib/core-oj.jar");
    // The failed pull leaves an empty dir that must NOT cover the .apex image...
    // (statsd was pulled; art failed; art's image is absent from the plan anyway)
    expect(result.skippedCoveredModules).toBe(0);

    rmSync(rootDir, { recursive: true, force: true });
  });
});
