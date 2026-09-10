import { Command } from "commander";
import { registerAndroidAppAnalysisCommands, registerAndroidResourceCommands } from "./android-app.js";
import { registerAndroidDeviceCommands } from "./android-device.js";
import { registerAndroidFrameworkCommands } from "./android-framework.js";
import { addClientConnectionOptions } from "./shared-options.js";

export function makeAndroidCommand(): Command {
  const cmd = new Command("android");
  cmd.description("Analyze Android apps and frameworks, or inspect a connected device");

  addClientConnectionOptions(cmd);

  registerAndroidAppAnalysisCommands(cmd);
  registerAndroidDeviceCommands(cmd);
  registerAndroidResourceCommands(cmd);
  registerAndroidFrameworkCommands(cmd);

  return cmd;
}
