import type { IncomingMessage, ServerResponse } from "node:http";
import { clientIp, handleSubscribe, readJsonBody, sendJson } from "../src/signup/http";

export default async function handler(req: IncomingMessage, res: ServerResponse): Promise<void> {
  if (req.method !== "POST") {
    res.statusCode = 405;
    res.setHeader("allow", "POST");
    res.end("Method not allowed");
    return;
  }

  try {
    const response = await handleSubscribe(await readJsonBody(req), {
      ip: clientIp(req),
      userAgent: req.headers["user-agent"],
    });
    sendJson(res, response);
  } catch {
    sendJson(res, {
      statusCode: 503,
      body: {
        ok: false,
        status: "temporary_failure",
        message: "Signup is temporarily unavailable.",
      },
    });
  }
}
