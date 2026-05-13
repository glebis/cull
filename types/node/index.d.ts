declare namespace NodeJS {
  interface ProcessEnv {
    [key: string]: string | undefined;
    TAURI_DEV_HOST?: string;
  }
}

declare const process: {
  env: NodeJS.ProcessEnv;
};
