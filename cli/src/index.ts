#!/usr/bin/env node

/**
 * JIAP CLI - Java Intelligence Analysis Platform
 */

import { Command } from "commander";
import { makeProcessCommand } from "./commands/process.js";
import { makeCodeCommand } from "./commands/code.js";
import { makeArdCommand } from "./commands/ard.js";

// Set up the Commander program with version info and register all command groups
const program = new Command();

program
  .name("jiap")
  .version("2.0.0")
  .description("JIAP CLI - Java Intelligence Analysis Platform")
  .addCommand(makeProcessCommand())
  .addCommand(makeCodeCommand())
  .addCommand(makeArdCommand());

program.parse();
