import { Resend } from "resend";

export type ConfirmationEmail = {
  to: string;
  confirmUrl: string;
};

const FOUNDER_NAME = "Gleb Kalinin";
const FOUNDER_LINKEDIN_URL = "https://www.linkedin.com/in/glebkalinin/";

export interface EmailSender {
  sendConfirmation(email: ConfirmationEmail): Promise<void>;
}

export class ResendEmailSender implements EmailSender {
  private resend: Resend;

  constructor(
    apiKey: string,
    private from: string,
  ) {
    this.resend = new Resend(apiKey);
  }

  async sendConfirmation(email: ConfirmationEmail): Promise<void> {
    const result = await this.resend.emails.send({
      from: this.from,
      to: email.to,
      subject: "Confirm your Cull launch update",
      html: buildConfirmationEmailHtml(email.confirmUrl),
      text: buildConfirmationEmailText(email.confirmUrl),
    });

    if (result.error) {
      throw new Error(result.error.message);
    }
  }
}

export function buildConfirmationEmailText(confirmUrl: string): string {
  return [
    "Confirm your Cull launch update",
    "",
    "Cull is a local-first desktop image review tool for people who shoot, generate, or produce at volume. It helps you go from hundreds of images to the keepers, with your files staying on your Mac.",
    "",
    `Open this link to confirm your email: ${confirmUrl}`,
    "",
    "You are not on the launch list until you confirm.",
    "",
    `Built by ${FOUNDER_NAME}: ${FOUNDER_LINKEDIN_URL}`,
  ].join("\n");
}

export function buildConfirmationEmailHtml(confirmUrl: string): string {
  const escapedConfirmUrl = escapeHtml(confirmUrl);
  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Confirm your Cull launch update</title>
  </head>
  <body style="margin:0; padding:0; background:#08080c; color:#e0e0e0; font-family:Arial, Helvetica, sans-serif;">
    <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="width:100%; background:#08080c; padding:32px 16px;">
      <tr>
        <td align="center">
          <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px; width:100%; background:#0c0c12; border:1px solid #1a1a2e; border-radius:8px; overflow:hidden;">
            <tr>
              <td style="padding:28px 28px 8px 28px;">
                <div style="color:#9ece6a; font-family:'Courier New', Courier, monospace; font-size:12px; letter-spacing:0.08em; text-transform:uppercase;">Cull launch update</div>
                <h1 style="margin:12px 0 0 0; color:#e0e0e0; font-size:28px; line-height:1.15; font-weight:700;">Confirm your email</h1>
              </td>
            </tr>
            <tr>
              <td style="padding:8px 28px 0 28px;">
                <p style="margin:0 0 16px 0; color:#c9cbda; font-size:16px; line-height:1.55;">Cull is a local-first desktop image review tool for people who shoot, generate, or produce at volume. It helps you go from hundreds of images to the keepers, with your files staying on your Mac.</p>
                <p style="margin:0 0 24px 0; color:#c9cbda; font-size:16px; line-height:1.55;">Confirm this address to get early builds and the open-source launch update.</p>
              </td>
            </tr>
            <tr>
              <td style="padding:0 28px 28px 28px;">
                <a href="${escapedConfirmUrl}" style="display:inline-block; background:#7aa2f7; color:#08080c; border-radius:6px; padding:14px 20px; font-size:16px; line-height:1; font-weight:700; text-decoration:none;">Confirm email</a>
                <p style="margin:18px 0 0 0; color:#7a7fa0; font-size:13px; line-height:1.45;">You are not on the launch list until you confirm.</p>
              </td>
            </tr>
            <tr>
              <td style="padding:20px 28px 28px 28px; border-top:1px solid #1a1a2e;">
                <p style="margin:0; color:#7a7fa0; font-size:13px; line-height:1.5;">Built by <a href="${FOUNDER_LINKEDIN_URL}" style="color:#9ece6a; text-decoration:underline;">${FOUNDER_NAME}</a>.</p>
                <p style="margin:14px 0 0 0; color:#7a7fa0; font-size:12px; line-height:1.5;">If the button does not work, open this link:<br><a href="${escapedConfirmUrl}" style="color:#7aa2f7; word-break:break-all;">${escapedConfirmUrl}</a></p>
              </td>
            </tr>
          </table>
        </td>
      </tr>
    </table>
  </body>
</html>`;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
