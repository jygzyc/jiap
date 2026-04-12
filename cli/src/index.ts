#!/usr/bin/env node

/**
 * JIAP CLI - Java Intelligence Analysis Platform
 */

import { readFileSync } from "fs";
import { fileURLToPath } from "url";
import { dirname, join } from "path";
import { Command } from "commander";
import { makeProcessCommand } from "./commands/process.js";
import { makeCodeCommand } from "./commands/code.js";
import { makeArdCommand } from "./commands/ard.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(readFileSync(join(__dirname, "../package.json"), "utf-8"));

const program = new Command();

program
  .name("jiap")
  .version(pkg.version)
  .description("JIAP CLI - Java Intelligence Analysis Platform")
  .addCommand(makeProcessCommand())
  .addCommand(makeCodeCommand())
  .addCommand(makeArdCommand());

program.parse();
