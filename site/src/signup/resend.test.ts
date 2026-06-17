import { describe, expect, it } from "vitest";
import { buildConfirmationEmailHtml, buildConfirmationEmailText } from "./resend";

describe("confirmation email rendering", () => {
  const confirmUrl = "https://cull.company/api/confirm?token=abc123&source=test";

  it("renders a styled HTML confirmation email", () => {
    const html = buildConfirmationEmailHtml(confirmUrl);

    expect(html).toContain("Confirm your email");
    expect(html).toContain("Cull is a local-first desktop image review tool");
    expect(html).toContain("Confirm email");
    expect(html).toContain("Gleb Kalinin");
    expect(html).toContain("https://www.linkedin.com/in/glebkalinin/");
    expect(html).toContain("https://cull.company/api/confirm?token=abc123&amp;source=test");
    expect(html).not.toContain("token=abc123&source=test");
  });

  it("renders a plain-text fallback with context and attribution", () => {
    const text = buildConfirmationEmailText(confirmUrl);

    expect(text).toContain("Confirm your Cull launch update");
    expect(text).toContain("Cull is a local-first desktop image review tool");
    expect(text).toContain(`Open this link to confirm your email: ${confirmUrl}`);
    expect(text).toContain("Built by Gleb Kalinin: https://www.linkedin.com/in/glebkalinin/");
  });
});
