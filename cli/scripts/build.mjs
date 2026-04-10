#!/usr/bin/env node

/**
 * JIAP CLI build script.
 *
 * 1. tsc — emit .d.ts declarations only (no JS, no source maps)
 * 2. esbuild — bundle, minify, and compress into dist/
 */

import { build } from "esbuild";
import { cpSync, rmSync, mkdirSync, readdirSync, statSync, existsSync } from "fs";
import { join, relative, dirname } from "path";
import { execSync } from "child_process";

const ROOT = join(dirname(new URL(import.meta.url).pathname), "..");
const DIST = join(ROOT, "dist");
const SRC = join(ROOT, "src");

// ── Step 1: Clean ──────────────────────────────────────────────────────────
rmSync(DIST, { recursive: true, force: true });

// ── Step 2: TypeScript declarations (d.ts only) ────────────────────────────
console.log("▸ Generating type declarations...");
execSync("npx tsc --emitDeclarationOnly --declaration --declarationMap false", {
  cwd: ROOT,
  stdio: "pipe",
});

// Move .d.ts files from dist/src to dist/src (keep structure), remove JS
function removeJsFiles(dir) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) {
      removeJsFiles(full);
    } else if (entry.endsWith(".js")) {
      rmSync(full);
    }
  }
}
removeJsFiles(DIST);

// ── Step 3: esbuild — bundle & minify ──────────────────────────────────────
console.log("▸ Bundling with esbuild...");

const entryPoint = join(SRC, "index.ts");

await build({
  entryPoints: [entryPoint],
  bundle: true,
  platform: "node",
  target: "node18",
  format: "esm",
  outfile: join(DIST, "index.js"),
  minify: true,
  treeShaking: true,
  packages: "external",          // don't bundle node_modules
  sourcemap: false,
  metafile: true,
  logLevel: "info",
});

// ── Step 4: Copy package.json (production only) ────────────────────────────
console.log("▸ Copying package.json...");
const pkg = JSON.parse(
  await import("fs").then(fs => fs.promises.readFile(join(ROOT, "package.json"), "utf-8"))
);
// Keep only production fields
const prodPkg = {
  name: pkg.name,
  version: pkg.version,
  description: pkg.description,
  type: pkg.type,
  bin: { jiap: "./index.js" },
  engines: pkg.engines,
  keywords: pkg.keywords,
  author: pkg.author,
  license: pkg.license,
  repository: pkg.repository,
  bugs: pkg.bugs,
  homepage: pkg.homepage,
  dependencies: { ...pkg.dependencies },
};
const { writeFileSync } = await import("fs");
writeFileSync(join(DIST, "package.json"), JSON.stringify(prodPkg, null, 2) + "\n");

// ── Done ───────────────────────────────────────────────────────────────────
console.log("✓ Build complete");
