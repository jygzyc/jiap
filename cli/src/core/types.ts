// Shared types for JIAP CLI

export interface Session {
  name: string;
  hash: string;
  pid: number;
  port: number;
  path: string;
  startedAt: number;
}

export interface JadxConfig {
  path: string | null;
  version: string;
  installDir: string;
}

export interface ServerConfig {
  defaultPort: number;
  timeout: number;
}

export interface OutputConfig {
  defaultDir: string;
  decompileDir: string;
}

export interface Config {
  jadx: JadxConfig;
  server: ServerConfig;
  output: OutputConfig;
}

export interface DecompileOptions {
  input_file: string;
  output_dir?: string;
  jadx_bin?: string;
  no_res?: boolean;
  threads?: number;
  timeout?: number;
  extra_args?: string[];
}

export interface ComponentCheckResult {
  ok: boolean;
  info: string;
}

export interface ComponentCheckResults {
  server: ComponentCheckResult;
  jadx: ComponentCheckResult;
  plugin: ComponentCheckResult;
}

export interface OemPaths {
  [oem: string]: string[];
}

export interface FrameworkFile {
  name: string;
  size_mb: number;
}

export interface FrameworkFiles {
  apks: FrameworkFile[];
  dexes: FrameworkFile[];
}
