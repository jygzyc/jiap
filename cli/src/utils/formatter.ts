/**
 * Colored CLI output formatter with optional JSON mode.
 *
 * In normal mode, prints structured data with indentation and color labels.
 * In JSON mode, outputs raw JSON (all non-data methods become no-ops).
 */

import pc from "picocolors";

export class Formatter {
  private jsonMode: boolean;

  constructor(jsonMode: boolean = false) {
    this.jsonMode = jsonMode;
  }

  /** Print data: raw JSON when in jsonMode, otherwise pretty-printed tree. */
  output(data: unknown): void {
    if (this.jsonMode) {
      console.log(JSON.stringify(data, null, 2));
    } else {
      this.printData(data, 0);
    }
  }

  /** Recursively pretty-print objects/arrays with indentation. */
  private printData(data: unknown, indent: number): void {
    const prefix = "  ".repeat(indent);

    if (data !== null && typeof data === "object") {
      if (Array.isArray(data)) {
        data.forEach((item, i) => {
          if (item !== null && typeof item === "object") {
            const name =
              (item as Record<string, unknown>).name ??
              (item as Record<string, unknown>)["class"] ??
              String(item);
            console.log(`${prefix}[${i}] ${name}`);
          } else {
            console.log(`${prefix}[${i}] ${item}`);
          }
        });
      } else {
        const obj = data as Record<string, unknown>;
        for (const [k, v] of Object.entries(obj)) {
          if (v !== null && typeof v === "object" && !Array.isArray(v)) {
            console.log(`${prefix}${k}:`);
            this.printData(v, indent + 1);
          } else if (Array.isArray(v) && v.length > 0) {
            console.log(`${prefix}${k}:`);
            this.printData(v, indent + 1);
          } else {
            console.log(`${prefix}${k}: ${v}`);
          }
        }
      }
    } else {
      console.log(`${prefix}${data}`);
    }
  }

  success(msg: string): void {
    if (!this.jsonMode) {
      console.log(pc.green(`  [OK] ${msg}`));
    }
  }

  error(msg: string): void {
    console.error(pc.red(`  [ERR] ${msg}`));
  }

  info(msg: string): void {
    if (!this.jsonMode) {
      console.log(pc.cyan(`  [*] ${msg}`));
    }
  }

  hint(msg: string): void {
    if (!this.jsonMode) {
      console.log(pc.magenta(`  > ${msg}`));
    }
  }

  warning(msg: string): void {
    if (!this.jsonMode) {
      console.log(pc.yellow(`  [!] ${msg}`));
    }
  }

  section(title: string): void {
    if (!this.jsonMode) {
      console.log(`\n  ${title}\n  ${"-".repeat(title.length)}`);
    }
  }
}
