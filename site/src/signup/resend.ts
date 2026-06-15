import { Resend } from "resend";

export type ConfirmationEmail = {
  to: string;
  confirmUrl: string;
};

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
      text: [
        "Confirm your Cull launch update:",
        "",
        email.confirmUrl,
        "",
        "You are not on the list until you open this link.",
      ].join("\n"),
    });

    if (result.error) {
      throw new Error(result.error.message);
    }
  }
}
