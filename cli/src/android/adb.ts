import { spawnSync } from "child_process";
import { DecxError, FileError, ProcessError } from "../utils/errors.js";
import type { AdbClientOptions, AdbCommandResult } from "./types.js";

const SUPPORTED_FRAMEWORK_OEMS = ["vivo", "oppo", "xiaomi", "honor", "google"] as const;
type SupportedFrameworkOem = typeof SUPPORTED_FRAMEWORK_OEMS[number];

export function parseAdbDevicesOutput(output: string): string[] {
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .filter((line) => !line.startsWith("List of devices attached"))
    .map((line) => line.split(/\s+/))
    .filter((parts) => parts.length >= 2 && parts[1] === "device")
    .map((parts) => parts[0]);
}

export function resolvePreferredSerial(output: string, requestedSerial?: string): string {
  if (requestedSerial) return requestedSerial;
  const devices = parseAdbDevicesOutput(output);
  if (devices.length === 0) {
    throw new DecxError("No connected Android device detected via adb", "ADB_DEVICE_MISSING");
  }
  if (devices.length > 1) {
    throw new DecxError(
      `Multiple adb devices detected (${devices.join(", ")}). Use --serial to select one.`,
      "ADB_DEVICE_AMBIGUOUS",
    );
  }
  return devices[0];
}

export function detectFrameworkOemFromBrand(brand: string): SupportedFrameworkOem {
  const normalized = brand.trim().toLowerCase();
  if ((SUPPORTED_FRAMEWORK_OEMS as readonly string[]).includes(normalized)) {
    return normalized as SupportedFrameworkOem;
  }
  throw new DecxError(
    `Unsupported device OEM '${brand}'. Supported: ${SUPPORTED_FRAMEWORK_OEMS.join(", ")}`,
    "ADB_UNSUPPORTED_OEM",
  );
}

export class AdbClient {
  private selectedSerial: string | null = null;

  constructor(private readonly options: AdbClientOptions = {}) {}

  get adbPath(): string {
    return this.options.adbPath ?? "adb";
  }

  private baseArgs(): string[] {
    const serial = this.selectedSerial ?? this.options.serial;
    return serial ? ["-s", serial] : [];
  }

  private run(args: string[], timeout: number = 300_000): AdbCommandResult {
    const result = spawnSync(this.adbPath, [...this.baseArgs(), ...args], {
      encoding: "utf-8",
      timeout,
    });

    if (result.error) {
      throw new FileError(`Failed to execute adb: ${result.error.message}`, this.adbPath);
    }

    return {
      stdout: result.stdout ?? "",
      stderr: result.stderr ?? "",
      status: result.status,
    };
  }

  ensureAvailable(): void {
    const result = this.run(["version"], 10_000);
    if (result.status !== 0) {
      throw new ProcessError(result.stderr.trim() || result.stdout.trim() || "adb is not available");
    }
  }

  private selectDevice(): void {
    this.selectedSerial = resolvePreferredSerial(this.run(["devices"], 10_000).stdout, this.options.serial);
  }

  ensureDeviceConnected(): void {
    this.selectDevice();
    const result = this.run(["get-state"], 10_000);
    if (result.status !== 0 || !result.stdout.includes("device")) {
      throw new DecxError("No connected Android device detected via adb", "ADB_DEVICE_MISSING");
    }
  }

  shell(command: string, timeout: number = 300_000): string {
    const result = this.run(["shell", command], timeout);
    if (result.status !== 0) {
      throw new ProcessError(result.stderr.trim() || result.stdout.trim() || `adb shell failed: ${command}`);
    }
    return result.stdout;
  }

  getProp(name: string): string {
    return this.shell(`getprop ${name}`, 10_000).trim();
  }

  detectFrameworkOem(): SupportedFrameworkOem {
    const brand =
      this.getProp("ro.product.vendor.brand")
      || this.getProp("ro.product.brand")
      || this.getProp("ro.product.manufacturer");
    return detectFrameworkOemFromBrand(brand);
  }

  pull(remotePath: string, localPath: string, timeout: number = 300_000): void {
    const result = this.run(["pull", remotePath, localPath], timeout);
    if (result.status !== 0) {
      throw new ProcessError(result.stderr.trim() || result.stdout.trim() || `adb pull failed: ${remotePath}`);
    }
  }
}
