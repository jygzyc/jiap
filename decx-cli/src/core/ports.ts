import { createServer } from "net";
import { ProcessError } from "../utils/errors.js";

export const MIN_SERVER_PORT = 1001;
export const MAX_SERVER_PORT = 65535;

/** Default range for randomly assigning a server port when none is requested. */
export const RANDOM_PORT_RANGE_MIN = 30000;
export const RANDOM_PORT_RANGE_MAX = 40000;

export function parseServerPort(value: string | number): number {
  const port = typeof value === "number"
    ? value
    : /^\d+$/.test(value)
      ? Number(value)
      : Number.NaN;

  if (!Number.isInteger(port) || port < MIN_SERVER_PORT || port > MAX_SERVER_PORT) {
    throw new ProcessError(`Invalid port: ${value}`);
  }

  return port;
}

async function canBindPort(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const server = createServer();

    server.once("error", () => resolve(false));
    server.listen({ port, host: "127.0.0.1", exclusive: true }, () => {
      server.close(() => resolve(true));
    });
  });
}

function randomPortInRange(min: number, max: number): number {
  return Math.floor(Math.random() * (max - min + 1)) + min;
}

/** Check whether a port is free to bind. */
export async function isServerPortAvailable(port: number): Promise<boolean> {
  port = parseServerPort(port);
  return canBindPort(port);
}

export async function selectAvailableServerPort(
  preferredPort: number | undefined,
): Promise<number> {
  // Honor an explicitly requested port when it is free.
  if (preferredPort !== undefined) {
    const port = parseServerPort(preferredPort);
    if (await isServerPortAvailable(port)) {
      return port;
    }
  }

  // Otherwise pick a random port in the default range until one is free.
  for (let i = 0; i < 100; i++) {
    const port = randomPortInRange(RANDOM_PORT_RANGE_MIN, RANDOM_PORT_RANGE_MAX);
    if (await isServerPortAvailable(port)) {
      return port;
    }
  }

  throw new ProcessError(
    `Failed to find an available port in [${RANDOM_PORT_RANGE_MIN}, ${RANDOM_PORT_RANGE_MAX}]`,
  );
}
