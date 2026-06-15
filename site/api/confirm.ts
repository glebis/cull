import type { IncomingMessage, ServerResponse } from "node:http";
import { handleConfirm, requireEnv } from "../src/signup/http";

export default async function handler(req: IncomingMessage, res: ServerResponse): Promise<void> {
  if (req.method !== "GET") {
    res.statusCode = 405;
    res.setHeader("allow", "GET");
    res.end("Method not allowed");
    return;
  }

  const url = new URL(req.url ?? "/", requireEnv("PUBLIC_SITE_URL"));
  const location = await handleConfirm(url.searchParams.get("token") ?? undefined);

  res.statusCode = 302;
  res.setHeader("location", location);
  res.end();
}
