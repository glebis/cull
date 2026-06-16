import "./styles.css";

document.querySelector<HTMLDivElement>("#app")!.innerHTML = `
  <div class="page-shell">
    <header class="topbar">
      <a class="mark" href="/" aria-label="Cull home">
        <img src="/images/cull-app-logo.png" alt="" />
        <span>Cull</span>
      </a>
      <span class="release-note">open source release in progress</span>
    </header>

    <section class="hero" aria-labelledby="hero-title">
      <div class="hero-copy">
        <p class="eyebrow">agent-first image culling</p>
        <h1 id="hero-title">A local-first image viewer for people and agents</h1>
        <p class="lede">Cull is a macOS desktop app for reviewing AI-generated image sets: fast keyboard decisions, real metadata, and an agent surface that speaks CLI and MCP.</p>
        <form class="signup-form" data-signup-form>
          <label for="email">Get the open-source launch update and early builds.</label>
          <div class="signup-row">
            <input id="email" name="email" type="email" autocomplete="email" placeholder="you@example.com" required />
            <button type="submit" data-submit-button>Request access</button>
          </div>
          <p class="form-status" data-form-status>Confirmed opt-in. No list until you click the email.</p>
        </form>
      </div>
      <figure class="product-shot">
        <img src="/images/cull-final-loupe.png" alt="Cull loupe view showing image review controls" />
      </figure>
    </section>

    <section class="claims" aria-label="Cull claims">
      <div>
        <span>01</span>
        <h2>Local-first</h2>
        <p>Your files stay on your Mac. Cull tracks review state in a local SQLite library and leaves originals untouched.</p>
      </div>
      <div>
        <span>02</span>
        <h2>Keyboard-first</h2>
        <p>Move through hundreds of images, rate them, accept or reject them, compare variants, and export without losing rhythm.</p>
      </div>
      <div>
        <span>03</span>
        <h2>Agent-first</h2>
        <p>The same work can be driven through app UI, token-efficient JSON CLI calls, and MCP tools for coding agents.</p>
      </div>
    </section>

    <section class="technical">
      <p>Tauri 2 / Rust / Svelte 5 / SQLite / ONNX embeddings / MCP / cull:// deep links</p>
    </section>

    <section class="founder-note">
      <p>I built Cull after learning enough Mac development and engineering discipline to make agents useful inside deterministic software. It is not a promise about magic. It is a tool for the boring part that makes image work faster: looking carefully, deciding clearly, and exporting the right set.</p>
    </section>

    <section class="bottom-signup" aria-label="Signup">
      <h2>Follow the release.</h2>
      <p>One confirmation email, then launch updates only.</p>
    </section>
  </div>
`;

const queryState = new URLSearchParams(window.location.search).get("signup");

if (queryState) {
  const messages: Record<string, { text: string; kind: StatusKind }> = {
    confirmed: { text: "Confirmed. You are on the Cull launch list.", kind: "success" },
    already_confirmed: { text: "Already confirmed. You are on the list.", kind: "success" },
    expired: { text: "That confirmation link expired. Submit your email again for a fresh link.", kind: "error" },
    invalid: { text: "That confirmation link is not valid. Submit your email again for a fresh link.", kind: "error" },
  };
  const message = messages[queryState];
  if (message) {
    setAllStatuses(message.text, message.kind);
  }
}

for (const form of document.querySelectorAll<HTMLFormElement>("[data-signup-form]")) {
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    const email = new FormData(form).get("email");
    const button = form.querySelector<HTMLButtonElement>("[data-submit-button]");

    setStatus(form, "Sending confirmation email...", "neutral");
    button?.setAttribute("disabled", "true");

    try {
      const response = await fetch("/api/subscribe", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ email }),
      });
      const body = (await response.json()) as { message?: string; ok?: boolean };
      setStatus(form, body.message ?? "Signup is temporarily unavailable.", body.ok ? "success" : "error");
      if (body.ok) {
        form.reset();
      }
    } catch {
      setStatus(form, "Signup is temporarily unavailable.", "error");
    } finally {
      button?.removeAttribute("disabled");
    }
  });
}

type StatusKind = "neutral" | "success" | "error";

function setAllStatuses(text: string, kind: StatusKind): void {
  for (const form of document.querySelectorAll<HTMLFormElement>("[data-signup-form]")) {
    setStatus(form, text, kind);
  }
}

function setStatus(form: HTMLFormElement, text: string, kind: StatusKind): void {
  const status = form.querySelector<HTMLElement>("[data-form-status]");
  if (!status) {
    return;
  }
  status.textContent = text;
  status.dataset.kind = kind;
}
