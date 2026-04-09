/**
 * Configuration validator for JIAP CLI.
 * Validates user inputs and configuration values.
 */

import * as path from "path";
import { JiapError } from "../utils/errors.js";

export class ValidationError extends JiapError {
  constructor(message: string) {
    super(message, "VALIDATION_ERROR");
    this.name = "ValidationError";
  }
}

export class Validator {
  /**
   * Validate OEM name.
   */
  static oem(oem: string): void {
    const validOems = ["vivo", "oppo", "xiaomi", "honor", "google"];
    if (!validOems.includes(oem.toLowerCase())) {
      throw new ValidationError(
        `Invalid OEM: ${oem}. Valid options: ${validOems.join(", ")}`
      );
    }
  }

  /**
   * Validate file extension.
   */
  static fileExtension(filePath: string, allowedExtensions: string[]): void {
    const ext = path.extname(filePath).toLowerCase();
    if (!allowedExtensions.includes(ext)) {
      throw new ValidationError(
        `Invalid file extension: ${ext}. Allowed: ${allowedExtensions.join(", ")}`
      );
    }
  }

}
