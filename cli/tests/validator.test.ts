/**
 * Validator unit tests.
 */

import { Validator, ValidationError } from "../src/core/validator.js";
import { JiapError } from "../src/utils/errors.js";

describe("Validator", () => {
  // ── oem ───────────────────────────────────────────────────────────────────

  describe("oem", () => {
    it("accepts valid OEMs", () => {
      for (const oem of ["vivo", "oppo", "xiaomi", "honor", "google"]) {
        expect(() => Validator.oem(oem)).not.toThrow();
      }
    });

    it("rejects invalid OEM (case-sensitive)", () => {
      expect(() => Validator.oem("samsung")).toThrow(ValidationError);
      expect(() => Validator.oem("huawei")).toThrow(ValidationError);
    });

    it("accepts case-insensitive valid OEMs", () => {
      expect(() => Validator.oem("VIVO")).not.toThrow();
      expect(() => Validator.oem("Xiaomi")).not.toThrow();
    });
  });

  // ── fileExtension ─────────────────────────────────────────────────────────

  describe("fileExtension", () => {
    it("accepts allowed extensions", () => {
      expect(() => Validator.fileExtension("test.apk", [".apk", ".dex"])).not.toThrow();
    });

    it("rejects disallowed extensions", () => {
      expect(() => Validator.fileExtension("test.exe", [".apk", ".dex"])).toThrow(ValidationError);
    });
  });
});
