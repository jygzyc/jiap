#!/usr/bin/env node

/**
 * PoC project bootstrap script.
 *
 * Usage: node setup-poc.mjs <target-app>
 *
 * Unzips the PoC template and replaces the placeholder package:
 *   com.poc.targetapp -> com.poc.<target-app>
 */

import { execSync } from 'node:child_process';
import {
  readFileSync,
  writeFileSync,
  readdirSync,
  statSync,
  renameSync,
} from 'node:fs';
import { join, dirname, basename, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));

const PLACEHOLDER_PACKAGE = 'com.poc.targetapp';
const PLACEHOLDER_PROJECT = 'poc-targetapp';
const TEXT_EXTENSIONS = new Set([
  '.java',
  '.kt',
  '.xml',
  '.gradle',
  '.properties',
  '.toml',
  '.md',
  '.txt',
  '.pro',
]);

function usage() {
  console.error('Usage: node setup-poc.mjs <target-app>');
  console.error('');
  console.error('Examples:');
  console.error('  node setup-poc.mjs myapp   -> creates poc-myapp/, package com.poc.myapp');
  console.error('  node setup-poc.mjs wechat  -> creates poc-wechat/, package com.poc.wechat');
  process.exit(1);
}

function validateAppName(name) {
  if (!name || !/^[a-z][a-z0-9]*$/.test(name)) {
    console.error(
      `Error: invalid app name "${name}". Use lowercase letters or digits, starting with a letter.`
    );
    process.exit(1);
  }
}

function unzip(src, dest) {
  execSync(`unzip -qo "${src}" -d "${dest}"`, { stdio: 'inherit' });
}

function isTextFile(filePath) {
  return TEXT_EXTENSIONS.has(extname(filePath));
}

function replaceInFile(filePath, replacements) {
  if (!isTextFile(filePath)) {
    return false;
  }

  let content = readFileSync(filePath, 'utf-8');
  let changed = false;

  for (const [from, to] of replacements) {
    if (content.includes(from)) {
      content = content.replaceAll(from, to);
      changed = true;
    }
  }

  if (changed) {
    writeFileSync(filePath, content, 'utf-8');
  }

  return changed;
}

function walkAndReplace(baseDir, replacements) {
  const entries = readdirSync(baseDir, { withFileTypes: true });
  let changedFiles = 0;

  for (const entry of entries) {
    const fullPath = join(baseDir, entry.name);
    if (entry.isDirectory()) {
      changedFiles += walkAndReplace(fullPath, replacements);
    } else if (replaceInFile(fullPath, replacements)) {
      changedFiles++;
    }
  }

  if (basename(baseDir) === 'targetapp') {
    const newPath = join(dirname(baseDir), replacements.targetSegment);
    renameSync(baseDir, newPath);
  }

  return changedFiles;
}

function collectRemainingMatches(baseDir, terms) {
  const matches = [];
  const entries = readdirSync(baseDir, { withFileTypes: true });

  for (const entry of entries) {
    const fullPath = join(baseDir, entry.name);
    if (entry.isDirectory()) {
      matches.push(...collectRemainingMatches(fullPath, terms));
      continue;
    }

    if (!isTextFile(fullPath)) {
      continue;
    }

    const content = readFileSync(fullPath, 'utf-8');
    if (terms.some((term) => content.includes(term))) {
      matches.push(fullPath);
    }
  }

  return matches;
}

function main() {
  const appName = process.argv[2];
  if (!appName) {
    usage();
  }

  validateAppName(appName);

  const newPackage = `com.poc.${appName}`;
  const projectDir = join(process.cwd(), `poc-${appName}`);
  const templateZip = join(__dirname, '..', 'assets', 'poc-template.zip');
  const replacements = new Map([
    [PLACEHOLDER_PACKAGE, newPackage],
    [PLACEHOLDER_PROJECT, `poc-${appName}`],
  ]);
  replacements.targetSegment = appName;

  try {
    statSync(templateZip);
  } catch {
    console.error(`Error: template not found: ${templateZip}`);
    process.exit(1);
  }

  try {
    statSync(projectDir);
    console.error(`Error: destination already exists: ${projectDir}`);
    console.error('Remove it first if you want to recreate the project.');
    process.exit(1);
  } catch {
    // destination does not exist
  }

  console.log('Initializing PoC project...');
  console.log(`  Target app: ${appName}`);
  console.log(`  Package: ${PLACEHOLDER_PACKAGE} -> ${newPackage}`);
  console.log(`  Output dir: ${projectDir}`);
  console.log('');

  console.log('  [1/3] Unzipping template...');
  unzip(templateZip, projectDir);

  console.log('  [2/3] Replacing placeholders...');
  const changedFiles = walkAndReplace(projectDir, replacements);

  console.log('  [3/3] Verifying...');
  const remaining = collectRemainingMatches(projectDir, [
    PLACEHOLDER_PACKAGE,
    PLACEHOLDER_PROJECT,
  ]);

  console.log('');
  console.log(`Created PoC project: ${projectDir}`);
  console.log(`Updated ${changedFiles} file(s)`);

  if (remaining.length > 0) {
    console.warn('');
    console.warn('Warning: unresolved placeholders remain in:');
    for (const file of remaining) {
      console.warn(`  - ${file}`);
    }
  }

  console.log('');
  console.log('Next steps:');
  console.log(`  cd ${projectDir}`);
  console.log('  ./gradlew assembleDebug');
}

main();
