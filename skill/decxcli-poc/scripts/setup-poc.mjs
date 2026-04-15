#!/usr/bin/env node

/**
 * PoC 项目初始化脚本
 *
 * 用法: node setup-poc.mjs <target-app>
 *
 * 从 poc-template.zip 解压并替换包名，生成 poc-<target-app>/ 项目。
 * 替换规则: com.poc.targetapp → com.poc.<target-app>（含目录名）
 */

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { join, dirname, basename } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const PLACEHOLDER = "com.poc.targetapp";

function usage() {
  console.error("用法: node setup-poc.mjs <target-app>");
  console.error("");
  console.error("示例:");
  console.error("  node setup-poc.mjs myapp      → 生成 poc-myapp/, 包名 com.poc.myapp");
  console.error("  node setup-poc.mjs wechat     → 生成 poc-wechat/, 包名 com.poc.wechat");
  process.exit(1);
}

function validateAppName(name) {
  if (!name || !/^[a-z][a-z0-9]*$/.test(name)) {
    console.error(`错误: app 名称 "${name}" 不合法，仅允许小写字母开头，后跟小写字母或数字`);
    process.exit(1);
  }
}

function unzip(src, dest) {
  execSync(`unzip -qo "${src}" -d "${dest}"`, { stdio: "inherit" });
}

function replaceInFile(filePath, from, to) {
  let content = readFileSync(filePath, "utf-8");
  if (content.includes(from)) {
    content = content.replaceAll(from, to);
    writeFileSync(filePath, content, "utf-8");
    return true;
  }
  return false;
}

function renameDirIfMatch(dirPath, segment, newSegment) {
  // dirPath 的最后一段如果是 segment，则重命名为 newSegment
  if (basename(dirPath) === segment) {
    const parent = dirname(dirPath);
    const newPath = join(parent, newSegment);
    execSync(`mv "${dirPath}" "${newPath}"`);
    return newPath;
  }
  return null;
}

function walkReplace(baseDir, from, to) {
  const entries = readdirSync(baseDir, { withFileTypes: true });
  // 先处理文件，再处理目录（目录重命名必须在内容替换之后）
  const files = [];
  const dirs = [];

  for (const entry of entries) {
    const fullPath = join(baseDir, entry.name);
    if (entry.isDirectory()) {
      dirs.push(fullPath);
    } else {
      files.push(fullPath);
    }
  }

  // 替换文件内容
  let fileCount = 0;
  for (const filePath of files) {
    if (replaceInFile(filePath, from, to)) {
      fileCount++;
    }
  }

  // 递归处理子目录
  for (const dirPath of dirs) {
    fileCount += walkReplace(dirPath, from, to);
  }

  // 重命名目录（从最深层往上，所以先递归再重命名）
  if (basename(baseDir) === "targetapp") {
    const parent = dirname(baseDir);
    const newPath = join(parent, to.split(".").pop()); // 最后一段
    execSync(`mv "${baseDir}" "${newPath}"`);
  }

  return fileCount;
}

function main() {
  const appName = process.argv[2];
  if (!appName) usage();
  validateAppName(appName);

  const newPkg = `com.poc.${appName}`;
  const projectDir = join(process.cwd(), `poc-${appName}`);
  const templateZip = join(__dirname, "..", "assets", "poc-template.zip");

  // 检查模板是否存在
  try {
    statSync(templateZip);
  } catch {
    console.error(`错误: 模板文件不存在: ${templateZip}`);
    process.exit(1);
  }

  // 检查目标目录是否已存在
  try {
    statSync(projectDir);
    console.error(`错误: 目录已存在: ${projectDir}`);
    console.error("如需重建，请先删除该目录");
    process.exit(1);
  } catch {
    // 不存在，继续
  }

  console.log(`初始化 PoC 项目...`);
  console.log(`  目标应用: ${appName}`);
  console.log(`  包名: ${PLACEHOLDER} → ${newPkg}`);
  console.log(`  输出目录: ${projectDir}`);
  console.log("");

  // 1. 解压模板
  console.log("  [1/3] 解压模板...");
  unzip(templateZip, projectDir);

  // 2. 替换文件内容 + 重命名目录
  console.log("  [2/3] 替换包名（文件内容 + 目录结构）...");
  const srcDir = join(projectDir, "app", "src", "main", "java");
  const fileCount = walkReplace(srcDir, PLACEHOLDER, newPkg);

  // 替换 build.gradle 中的包名（不在 java 目录下，单独处理）
  const buildGradle = join(projectDir, "app", "build.gradle");
  replaceInFile(buildGradle, PLACEHOLDER, newPkg);

  // 3. 验证
  console.log("  [3/3] 验证...");
  const remaining = execSync(
    `grep -r "${PLACEHOLDER}" "${projectDir}" --include="*.java" --include="*.gradle" --include="*.xml" --include="*.properties" -l 2>/dev/null || true`
  ).toString().trim();
  if (remaining) {
    console.warn(`  ⚠ 仍有未替换的占位符:`);
    console.warn(remaining);
  }

  console.log("");
  console.log(`✓ PoC 项目已创建: ${projectDir}`);
  console.log(`  替换了 ${fileCount} 个 Java 文件中的包名`);
  console.log("");
  console.log(`下一步:`);
  console.log(`  cd ${projectDir}`);
  console.log(`  ./gradlew assembleDebug`);
}

main();
