/**
 * Error handling utilities for JIAP CLI.
 * Provides structured error types and handling helpers.
 */

import { Formatter } from "./formatter.js";

/**
 * Base error class for JIAP CLI.
 */
export class JiapError extends Error {
  constructor(
    message: string,
    public readonly code?: string,
    public readonly details?: Record<string, unknown>
  ) {
    super(message);
    this.name = "JiapError";
  }

  format(fmt: Formatter): void {
    fmt.error(this.message);
    if (this.details && Object.keys(this.details).length > 0) {
      fmt.hint(JSON.stringify(this.details, null, 2));
    }
  }
}

/**
 * Process-related errors.
 */
export class ProcessError extends JiapError {
  constructor(message: string, pid?: number) {
    super(message, "PROCESS_ERROR", { pid });
    this.name = "ProcessError";
  }
}

/**
 * Server connection errors.
 */
export class ServerError extends JiapError {
  constructor(message: string, port?: number) {
    super(message, "SERVER_ERROR", { port });
    this.name = "ServerError";
  }
}

/**
 * File operation errors.
 */
export class FileError extends JiapError {
  constructor(message: string, filePath?: string) {
    super(message, "FILE_ERROR", { filePath });
    this.name = "FileError";
  }
}

/**
 * Configuration errors.
 */
export class ConfigError extends JiapError {
  constructor(message: string, key?: string) {
    super(message, "CONFIG_ERROR", { key });
    this.name = "ConfigError";
  }
}

/**
 * Error handler for CLI commands.
 */
export function handleCliError(error: unknown, formatter: Formatter): never {
  if (error instanceof JiapError) {
    error.format(formatter);
  } else if (error instanceof Error) {
    formatter.error(error.message);
    if (process.env.JIAP_DEBUG === "1") {
      console.error(error.stack);
    }
  } else {
    formatter.error(String(error));
  }
  process.exit(1);
}

/**
 * Wrap async command handler with error handling.
 */
export function withErrorHandler<T extends unknown[], R>(
  fn: (...args: T) => Promise<R>,
  formatterOrFactory: Formatter | ((...args: T) => Formatter)
): (...args: T) => Promise<R | void> {
  return async (...args: T) => {
    try {
      return await fn(...args);
    } catch (error) {
      const fmt = typeof formatterOrFactory === "function"
        ? formatterOrFactory(...args)
        : formatterOrFactory;
      handleCliError(error, fmt);
    }
  };
}

/**
 * Assert condition and throw formatted error if false.
 */
export function assert(
  condition: boolean,
  message: string,
  ErrorClass: typeof JiapError = JiapError
): asserts condition {
  if (!condition) {
    throw new ErrorClass(message);
  }
}
