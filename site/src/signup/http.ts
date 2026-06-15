import type { IncomingMessage, ServerResponse } from "node:http";
import { confirm, subscribe, type SignupConfig } from "./service";
import { ResendEmailSender, type EmailSender } from "./resend";
import { VercelBlobSignupStore, type SignupStore } from "./store";
import type { ConfirmResult, SubscribeResult } from "./service";

export type JsonResponse = {
  statusCode: number;
  body: {
    ok: boolean;
    status: string;
    message: string;
    retryAfterSeconds?: number;
  };
};

export type SignupDeps = {
  store: SignupStore;
  sender: EmailSender;
  config: SignupConfig;
};

export async function handleSubscribe(
  body: unknown,
  meta: { ip?: string; userAgent?: string },
  deps = productionDeps(),
): Promise<JsonResponse> {
  const email = readEmail(body);
  const result = await subscribe(
    {
      email,
      ip: meta.ip,
      userAgent: meta.userAgent,
      source: "landing",
    },
    deps.store,
    deps.sender,
    deps.config,
  );

  return subscribeResponse(result);
}

export async function handleConfirm(token: string | undefined, deps = productionDeps()): Promise<string> {
  const result = await confirm(token, deps.store, deps.config);
  return confirmRedirect(result);
}

export async function readJsonBody(req: IncomingMessage): Promise<unknown> {
  const chunks: Buffer[] = [];
  for await (const chunk of req) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }

  if (chunks.length === 0) {
    return {};
  }

  return JSON.parse(Buffer.concat(chunks).toString("utf8")) as unknown;
}

export function sendJson(res: ServerResponse, response: JsonResponse): void {
  res.statusCode = response.statusCode;
  res.setHeader("content-type", "application/json; charset=utf-8");
  if (response.body.retryAfterSeconds) {
    res.setHeader("retry-after", String(response.body.retryAfterSeconds));
  }
  res.end(JSON.stringify(response.body));
}

export function clientIp(req: IncomingMessage): string | undefined {
  const forwarded = req.headers["x-forwarded-for"];
  if (typeof forwarded === "string") {
    return forwarded.split(",")[0]?.trim();
  }
  return req.socket.remoteAddress;
}

export function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing ${name}`);
  }
  return value;
}

function productionDeps(): SignupDeps {
  return {
    store: new VercelBlobSignupStore(),
    sender: new ResendEmailSender(requireEnv("RESEND_API_KEY"), requireEnv("RESEND_FROM")),
    config: {
      secret: requireEnv("SIGNUP_SECRET"),
      siteUrl: requireEnv("PUBLIC_SITE_URL"),
    } satisfies SignupConfig,
  };
}

function readEmail(body: unknown): string {
  if (!body || typeof body !== "object" || typeof (body as { email?: unknown }).email !== "string") {
    return "";
  }
  return (body as { email: string }).email;
}

function subscribeResponse(result: SubscribeResult): JsonResponse {
  switch (result.status) {
    case "pending":
      return {
        statusCode: 200,
        body: {
          ok: true,
          status: result.status,
          message: "Check your email to confirm.",
        },
      };
    case "already_confirmed":
      return {
        statusCode: 200,
        body: {
          ok: true,
          status: result.status,
          message: "You are already on the list.",
        },
      };
    case "invalid_email":
      return {
        statusCode: 400,
        body: {
          ok: false,
          status: result.status,
          message: "Enter a valid email address.",
        },
      };
    case "rate_limited":
      return {
        statusCode: 429,
        body: {
          ok: false,
          status: result.status,
          message: "Too many attempts. Try again in a little while.",
          retryAfterSeconds: result.retryAfterSeconds,
        },
      };
    default:
      return {
        statusCode: 503,
        body: {
          ok: false,
          status: "temporary_failure",
          message: "Signup is temporarily unavailable.",
        },
      };
  }
}

function confirmRedirect(result: ConfirmResult): string {
  switch (result.status) {
    case "confirmed":
      return "/?signup=confirmed";
    case "already_confirmed":
      return "/?signup=already_confirmed";
    case "expired_token":
      return "/?signup=expired";
    default:
      return "/?signup=invalid";
  }
}
