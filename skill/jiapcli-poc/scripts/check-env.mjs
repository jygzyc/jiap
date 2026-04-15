#!/usr/bin/env node

/**
 * PoC 构建环境检测脚本
 *
 * 用法: node check-env.mjs
 *
 * 检测 Android SDK、JDK、Gradle 等构建环境是否就绪。
 * 输出各项检测结果，不自动修复任何问题。
 */

import { execSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

const checks = [];
let hasError = false;

function check(name, fn) {
  try {
    const result = fn();
    checks.push({ name, status: "ok", detail: result });
  } catch (e) {
    hasError = true;
    checks.push({ name, status: "fail", detail: e.message });
  }
}

// 1. ANDROID_HOME
check("ANDROID_HOME", () => {
  const home = process.env.ANDROID_HOME;
  if (!home) throw new Error("环境变量未设置");
  if (!existsSync(home)) throw new Error(`目录不存在: ${home}`);
  return home;
});

// 2. build-tools
check("SDK build-tools", () => {
  const home = process.env.ANDROID_HOME;
  if (!home) throw new Error("ANDROID_HOME 未设置");
  const dir = join(home, "build-tools");
  if (!existsSync(dir)) throw new Error(`目录不存在: ${dir}`);
  const versions = readdirSync(dir);
  if (versions.length === 0) throw new Error("目录为空");
  return versions.join(", ");
});

// 3. platforms
check("SDK platforms", () => {
  const home = process.env.ANDROID_HOME;
  if (!home) throw new Error("ANDROID_HOME 未设置");
  const dir = join(home, "platforms");
  if (!existsSync(dir)) throw new Error(`目录不存在: ${dir}`);
  const versions = readdirSync(dir);
  if (versions.length === 0) throw new Error("目录为空");
  return versions.join(", ");
});

// 4. JDK
check("JDK (java)", () => {
  const output = execSync("java -version 2>&1", { encoding: "utf-8" });
  const match = output.match(/version "(\d+)/);
  if (!match) throw new Error("无法解析 java 版本");
  const major = parseInt(match[1], 10);
  if (major < 11) throw new Error(`JDK ${major} 低于最低要求 11`);
  return match[0];
});

// 5. javac
check("JDK (javac)", () => {
  const output = execSync("javac -version 2>&1", { encoding: "utf-8" });
  const match = output.match(/javac (\d+)/);
  if (!match) throw new Error("无法解析 javac 版本");
  const major = parseInt(match[1], 10);
  if (major < 11) throw new Error(`javac ${major} 低于最低要求 11`);
  return match[0];
});

// 6. adb（可选）
check("adb (可选)", () => {
  const output = execSync("adb version 2>&1", { encoding: "utf-8" });
  return output.split("\n")[0];
});

// --- 输出结果 ---

console.log("");
console.log("PoC 构建环境检测");
console.log("─".repeat(50));

for (const c of checks) {
  const icon = c.status === "ok" ? "✓" : "✗";
  const label = c.detail || "";
  console.log(`  ${icon} ${c.name}`);
  if (label) console.log(`    ${label}`);
}

console.log("─".repeat(50));

if (hasError) {
  console.log("环境检测未通过，无法编译。");
  process.exit(1);
} else {
  console.log("环境检测通过，可以编译。");
  process.exit(0);
}
