import { createHmac, randomBytes } from "node:crypto";

export function generateToken(): string {
  return randomBytes(32).toString("base64url");
}

export function hmac(value: string, secret: string): string {
  if (secret.length < 24) {
    throw new Error("SIGNUP_SECRET must be at least 24 characters");
  }

  return createHmac("sha256", secret).update(value).digest("hex");
}

export function hashEmail(email: string, secret: string): string {
  return hmac(`email:${email}`, secret);
}

export function hashToken(token: string, secret: string): string {
  return hmac(`token:${token}`, secret);
}

export function hashIp(ip: string, secret: string): string {
  return hmac(`ip:${ip}`, secret);
}
