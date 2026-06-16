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
        <p class="eyebrow">local-first image culling</p>
        <h1 id="hero-title">Cull image batches without giving up the archive</h1>
        <p class="lede">Review generations, compare variants, preserve prompt context, and export clean sets. Cull stays on your Mac while agents get the same surface through CLI and MCP.</p>
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
        <img src="/images/cull-state-preview.png" alt="Cull app state with batch counts, image decisions, prompt metadata, and agent queue" />
      </figure>
    </section>

    <section class="claims" aria-label="Cull claims">
      <div>
        <h2>Local library</h2>
        <p>Review state lives in SQLite with SHA-256 deduplication. Originals stay untouched.</p>
      </div>
      <div>
        <h2>Keyboard decisions</h2>
        <p>Rate, accept, reject, compare, collect, and jump through the Command Palette without losing rhythm.</p>
      </div>
      <div>
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
          <p>Local CLIP and DINOv2 embeddings support similarity search, UMAP exploration, and clustering.</p>
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

    <section class="founder-note" aria-labelledby="release-title">
      <h2 id="release-title">Open source release in progress.</h2>
      <p>Cull is for AI artists, creative technologists, designers, researchers, and agentic workflow builders who manage large local archives of generated images, references, prompts, ratings, selections, and collections.</p>
      <p>It is not a promise about magic. It is a tool for the boring part that makes image work faster: looking carefully, deciding clearly, and keeping enough context to use the result later.</p>
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
