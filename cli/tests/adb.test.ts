import { detectFrameworkOemFromBrand, parseAdbDevicesOutput, resolvePreferredSerial } from "../src/android/adb.js";

describe("adb device selection", () => {
  it("parses connected devices from adb devices output", () => {
    const output = [
      "List of devices attached",
      "emulator-5554\tdevice",
      "ABC123\tdevice",
      "",
    ].join("\n");

    expect(parseAdbDevicesOutput(output)).toEqual(["emulator-5554", "ABC123"]);
  });

  it("defaults to the only connected device when serial is not provided", () => {
    const output = [
      "List of devices attached",
      "emulator-5554\tdevice",
      "",
    ].join("\n");

    expect(resolvePreferredSerial(output)).toBe("emulator-5554");
  });

  it("requires --serial when multiple devices are connected", () => {
    const output = [
      "List of devices attached",
      "emulator-5554\tdevice",
      "ABC123\tdevice",
      "",
    ].join("\n");

    expect(() => resolvePreferredSerial(output)).toThrow("Multiple adb devices detected");
  });

  it("uses the requested serial when provided", () => {
    const output = [
      "List of devices attached",
      "emulator-5554\tdevice",
      "ABC123\tdevice",
      "",
    ].join("\n");

    expect(resolvePreferredSerial(output, "ABC123")).toBe("ABC123");
  });

  it("detects a supported framework OEM directly from brand", () => {
    expect(detectFrameworkOemFromBrand("Xiaomi")).toBe("xiaomi");
    expect(detectFrameworkOemFromBrand("GOOGLE")).toBe("google");
  });
});
