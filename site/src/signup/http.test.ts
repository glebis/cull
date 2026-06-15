import { describe, expect, it } from "vitest";
import { handleConfirm, handleSubscribe, requireEnv } from "./http";
import type { EmailSender } from "./resend";
import type { SignupConfig } from "./service";
import { MemorySignupStore } from "./store";

const config: SignupConfig = {
  secret: "0123456789abcdef0123456789abcdef",
  siteUrl: "https://cull.company",
};

const sender: EmailSender = {
  async sendConfirmation() {
    return undefined;
  },
};

describe("handleSubscribe", () => {
  it("maps successful subscribe to a JSON response", async () => {
    const response = await handleSubscribe(
      { email: "gleb@example.com" },
      { ip: "127.0.0.1", userAgent: "vitest" },
      { store: new MemorySignupStore(), sender, config },
    );

    expect(response).toEqual({
      statusCode: 200,
      body: {
        ok: true,
        status: "pending",
        message: "Check your email to confirm.",
      },
    });
  });

  it("maps invalid email to 400", async () => {
    const response = await handleSubscribe({}, {}, { store: new MemorySignupStore(), sender, config });

    expect(response.statusCode).toBe(400);
    expect(response.body.status).toBe("invalid_email");
  });
});

describe("handleConfirm", () => {
  it("maps invalid tokens to a page query state", async () => {
    await expect(handleConfirm(undefined, { store: new MemorySignupStore(), sender, config })).resolves.toBe(
      "/?signup=invalid",
    );
  });
});

describe("requireEnv", () => {
  it("throws when an env var is missing", () => {
    const previous = process.env.MISSING_FOR_TEST;
    delete process.env.MISSING_FOR_TEST;

    expect(() => requireEnv("MISSING_FOR_TEST")).toThrow("Missing MISSING_FOR_TEST");

    process.env.MISSING_FOR_TEST = previous;
  });
});
