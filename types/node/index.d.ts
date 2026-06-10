declare namespace NodeJS {
  interface ProcessEnv {
    [key: string]: string | undefined;
    TAURI_DEV_HOST?: string;
  }
}

declare const process: {
  env: NodeJS.ProcessEnv;
  cwd(): string;
};

declare module 'node:fs' {
  export function existsSync(path: string): boolean;
  export function readFileSync(path: string, encoding: 'utf8'): string;
  export function readdirSync(path: string): string[];
  export function statSync(path: string): { isDirectory(): boolean };
}

declare module 'node:fs/promises' {
  export function chmod(path: string, mode: number): Promise<void>;
  export function mkdir(path: string): Promise<void>;
  export function mkdtemp(prefix: string): Promise<string>;
  export function readFile(path: string, encoding: 'utf8'): Promise<string>;
  export function writeFile(path: string, data: string): Promise<void>;
}

declare module 'node:child_process' {
  export type ExecFileOptions = {
    cwd?: string;
    env?: Record<string, string | undefined>;
    timeout?: number;
    maxBuffer?: number;
  };

  export function execFile(
    file: string,
    args: string[],
    options: ExecFileOptions,
    callback: (error: Error | null, stdout: string, stderr: string) => void,
  ): void;

  export function execFileSync(
    file: string,
    args: string[],
    options: { encoding: 'utf8' },
  ): string;
}

declare module 'node:http' {
  export type AddressInfo = { port: number };
  export type ServerResponse = {
    writeHead(statusCode: number, headers: Record<string, string>): void;
    end(body?: string): void;
  };

  export type Server = {
    once(event: 'error', listener: (error: Error) => void): Server;
    listen(port: number, host: string, callback: () => void): Server;
    address(): AddressInfo | string | null;
    close(callback?: (error?: Error) => void): Server;
  };

  export function createServer(listener: (request: unknown, response: ServerResponse) => void): Server;
}

declare module 'node:os' {
  export function tmpdir(): string;
}

declare module 'node:path' {
  export function dirname(path: string): string;
  export function join(...paths: string[]): string;
  export function resolve(...paths: string[]): string;
}

declare module 'node:url' {
  export function fileURLToPath(url: string | URL): string;
}
