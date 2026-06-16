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
        <p class="eyebrow">local-first image viewer</p>
        <h1 id="hero-title">A local-first image viewer for people and agents</h1>
        <p class="lede">Review large batches, compare variants, keep prompt context, and export a clean set. The archive stays on your Mac; the same work can be driven by the app, CLI, or MCP.</p>
        <form class="signup-form signup-form--featured" data-signup-form>
          <label for="email">Get the open-source launch update and early builds.</label>
          <div class="signup-row">
            <input id="email" name="email" type="email" autocomplete="email" placeholder="you@example.com" aria-describedby="signup-status" required />
            <button type="submit" data-submit-button>Get notified</button>
          </div>
          <p id="signup-status" class="form-status" data-form-status aria-live="polite">One confirmation email first. No imported lists.</p>
        </form>
      </div>
      <figure class="product-shot">
        <img src="/images/cull-state-preview.png" alt="App state with batch counts, image decisions, prompt metadata, and agent queue" />
      </figure>
    </section>

    <section class="claims" aria-label="Core claims">
      <div class="claim">
        <figure class="claim-illustration">
          <img src="/images/claim-local-library.png" alt="" />
        </figure>
        <h2>Local library</h2>
        <p>Review state lives in SQLite with SHA-256 deduplication. Originals stay untouched.</p>
      </div>
      <div class="claim">
        <figure class="claim-illustration">
          <img src="/images/claim-keyboard-decisions.png" alt="" />
        </figure>
        <h2>Keyboard decisions</h2>
        <p>Rate, accept, reject, compare, collect, and jump through the Command Palette without losing rhythm.</p>
      </div>
      <div class="claim">
        <figure class="claim-illustration">
          <img src="/images/claim-agent-surface.png" alt="" />
        </figure>
        <h2>Agent surface</h2>
        <p>CLI, MCP, and cull:// deep links share the same command model as the app.</p>
      </div>
    </section>

    <section class="workflow" aria-labelledby="workflow-title">
      <div>
        <p class="eyebrow">from folder to final set</p>
        <h2 id="workflow-title">Built for large local image libraries.</h2>
      </div>
      <div class="workflow-list">
        <article>
          <h3>Import the mess</h3>
          <p>Recursive folders, generated sidecars, thumbnails, file associations, and drag-and-drop from Finder.</p>
        </article>
        <article>
          <h3>Review in context</h3>
          <p>Grid, loupe, compare, canvas, lineage, embedding views, and fuzzy command search keep decisions close to metadata.</p>
        </article>
        <article>
          <h3>Search by meaning</h3>
          <p>Find images that look or feel related, surface nearby variations, and see clusters when a folder is too large to scan by hand. (Local CLIP, DINOv2, UMAP)</p>
        </article>
        <article>
          <h3>Export the result</h3>
          <p>Keepers can move to social exports, static publishing packages, and agent-readable snapshots.</p>
        </article>
      </div>
    </section>

    <section class="technical">
      <p>Tauri 2 / Rust / Svelte 5 / SQLite / ONNX Runtime / MCP / headless JSON CLI / cull:// deep links</p>
    </section>

    <section class="experience-note" aria-labelledby="experience-title">
      <div>
        <p class="eyebrow">jobs to be done</p>
        <h2 id="experience-title">Make the review feel smaller.</h2>
      </div>
      <div class="experience-copy">
        <p>When a generation run leaves hundreds of near-misses, the job is not to admire the grid. It is to find the few images worth carrying forward.</p>
        <p>Keep the originals intact. Keep enough context to return later. Let agents help with mechanical work without taking the eye out of the loop.</p>
      </div>
    </section>

    <section class="open-source-note" aria-labelledby="open-source-title">
      <figure class="open-source-illustration">
        <img src="/images/open-source-agents.png" alt="" />
      </figure>
      <div class="open-source-copy">
        <p class="eyebrow">open source</p>
        <h2 id="open-source-title">Open source, but still authored.</h2>
        <p>Released under Apache-2.0. The project is built with human product direction and multiple coding agents, with human review over architecture, copy, release choices, and the resulting intellectual property.</p>
        <p>Current repository history: <strong>653 commits</strong> and counting.</p>
      </div>
    </section>

    <section class="bottom-signup" aria-labelledby="privacy-title">
      <div>
        <p class="eyebrow">confirmed opt-in</p>
        <h2 id="privacy-title">Get notified when the release is ready.</h2>
      </div>
      <p>The launch list starts after you confirm the email. No imported lists, no background newsletter drift.</p>
    </section>

    <figure class="footer-illustration" aria-label="Image workflow from local archive to agent surfaces">
      <img src="/images/footer-line-map.png" alt="" />
    </figure>

    <footer class="site-footer">
      <p>Local-first release, confirmed opt-in, open-source code.</p>
      <nav aria-label="Footer links">
        <a href="https://github.com/glebis">Gleb GitHub</a>
        <a href="https://github.com/glebis/cull">Repository</a>
        <a href="https://t.me/glebkalinin">Telegram</a>
        <a href="https://www.linkedin.com/in/glebkalinin/">LinkedIn</a>
      </nav>
    </footer>
  </div>
`;

const queryState = new URLSearchParams(window.location.search).get("signup");

if (queryState) {
  const messages: Record<string, { text: string; kind: StatusKind }> = {
    confirmed: { text: "Confirmed. You are on the launch list.", kind: "success" },
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
