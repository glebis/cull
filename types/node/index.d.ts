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
  export function readFileSync(path: string, encoding: 'utf8'): string;
  export function readdirSync(path: string): string[];
  export function statSync(path: string): { isDirectory(): boolean };
}

declare module 'node:path' {
  export function join(...paths: string[]): string;
}
