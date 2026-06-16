import "./styles.css";

document.querySelector<HTMLDivElement>("#app")!.innerHTML = `
  <div class="page-shell">
    <header class="topbar">
      <a class="mark" href="/" aria-label="Cull home">
        <img src="/images/cull-app-logo.png" alt="" />
      </a>
      <span class="release-note">early release / open source</span>
    </header>

    <section class="hero" aria-labelledby="hero-title">
      <div class="hero-copy">
        <p class="eyebrow hero-step-1">local-first / agent-ready</p>
        <h1 id="hero-title" class="hero-step-2">Go from 500 images to 20 keepers</h1>
        <p class="lede hero-step-3">Cull is a fast image review tool for people who shoot, generate, or produce at volume. Your files stay on your Mac. Work in the app, or drive the work through your agent via CLI or MCP.</p>
        <form class="signup-form signup-form--featured hero-step-5" data-signup-form>
          <label for="email">Get early builds and the open-source launch update.</label>
          <div class="signup-row">
            <input id="email" name="email" type="email" autocomplete="email" placeholder="you@example.com" aria-describedby="signup-status" required />
            <button type="submit" data-submit-button>Get notified</button>
          </div>
          <p id="signup-status" class="form-status" data-form-status aria-live="polite">One confirmation email. No lists, no noise.</p>
        </form>
        <div class="hero-experience hero-step-6">
          <h2>The boring part of creative work, made fast</h2>
          <p>Most tools are built for editing. This is built for the moment before that, when you have hundreds of images and need to reach a final set.</p>
          <p>Look carefully, decide clearly, and make your work available to people and agents without friction.</p>
        </div>
      </div>
      <figure class="product-shot hero-step-4">
        <img src="/images/cull-state-preview.png" alt="App state with batch counts, image decisions, prompt metadata, and agent queue" />
      </figure>
    </section>

    <section class="claims reveal-surface" aria-label="Core claims" data-reveal>
      <div class="claim reveal-item reveal-delay-0">
        <figure class="claim-illustration">
          <img src="/images/claim-local-library.png" alt="" />
        </figure>
        <h2>Your library, your machine</h2>
        <p>Images stay local and private. Originals stay untouched. No upload or cloud account required.</p>
      </div>
      <div class="claim reveal-item reveal-delay-1">
        <figure class="claim-illustration">
          <img src="/images/claim-keyboard-decisions.png" alt="" />
        </figure>
        <h2>Decide in a keystroke</h2>
        <p>Rate, accept, reject, compare, and collect without lifting your hands from the keyboard.</p>
      </div>
      <div class="claim reveal-item reveal-delay-2">
        <figure class="claim-illustration">
          <img src="/images/claim-agent-surface.png" alt="" />
        </figure>
        <h2>AI can help when you want it</h2>
        <p>Sort a folder yourself, or hand it to your agent through CLI or MCP when you want help.</p>
      </div>
    </section>

    <section class="workflow reveal-surface" aria-labelledby="workflow-title" data-reveal>
      <div class="reveal-item reveal-delay-0">
        <p class="eyebrow">how it works</p>
        <h2 id="workflow-title">From folder to final set</h2>
      </div>
      <div class="workflow-list">
        <article class="reveal-item reveal-delay-1">
          <h3>Drop in your folder</h3>
          <p>Drag in any folder, any size, any structure. The app reads everything and stays out of the way.</p>
        </article>
        <article class="reveal-item reveal-delay-2">
          <h3>See every shot clearly</h3>
          <p>Grid, loupe, and side-by-side compare. Move through images at whatever pace works.</p>
        </article>
        <article class="reveal-item reveal-delay-3">
          <h3>Find what you are looking for</h3>
          <p>Search by look and feel rather than filename. Surface the sharp ones, the warm ones, or the ones that match a reference.</p>
        </article>
        <article class="reveal-item reveal-delay-4">
          <h3>Send out the keepers</h3>
          <p>Export picks for social, publishing, clients, or the next agent-assisted step.</p>
        </article>
      </div>
    </section>

    <section class="open-source-note reveal-surface" aria-labelledby="open-source-title" data-reveal>
      <figure class="open-source-illustration reveal-item reveal-delay-0">
        <img src="/images/open-source-agents.png" alt="" />
      </figure>
      <div class="open-source-copy reveal-item reveal-delay-2">
        <p class="eyebrow">built in the open</p>
        <h2 id="open-source-title">Open source by design</h2>
        <p>Free and open source. The code is public, your files stay on your machine, and no company can pull the tool away from you. Built with human direction and AI coding help, then reviewed and shipped by a person.</p>
        <p>Current repository history: <strong>653 commits</strong> and counting.</p>
      </div>
    </section>

    <section class="bottom-signup reveal-surface" aria-labelledby="privacy-title" data-reveal>
      <div class="reveal-item reveal-delay-0">
        <p class="eyebrow">early access</p>
        <h2 id="privacy-title">Be first when it ships</h2>
      </div>
      <p class="reveal-item reveal-delay-2">One confirmation email. That is it.</p>
    </section>

    <figure class="footer-illustration reveal-surface" aria-label="Image workflow from local archive to agent surfaces" data-reveal>
      <img src="/images/footer-line-map.png" alt="" />
    </figure>

    <footer class="site-footer">
      <p>Made in Berlin with 🇪🇺 by <a href="https://www.linkedin.com/in/glebkalinin/">Gleb Kalinin</a></p>
      <nav aria-label="Footer links">
        <a href="https://github.com/glebis">Gleb GitHub</a>
        <a href="https://github.com/glebis/cull">Repository</a>
        <a href="https://t.me/glebkalinin">Telegram</a>
      </nav>
    </footer>
  </div>
`;

const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");

if (motionQuery.matches) {
  document.documentElement.classList.add("reduced-motion");
}

const revealTargets = document.querySelectorAll<HTMLElement>("[data-reveal]");

if (!motionQuery.matches && "IntersectionObserver" in window) {
  const revealObserver = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) {
          continue;
        }
        entry.target.classList.add("is-visible");
        revealObserver.unobserve(entry.target);
      }
    },
    { rootMargin: "0px 0px -14% 0px", threshold: 0.16 },
  );

  for (const target of revealTargets) {
    revealObserver.observe(target);
  }
} else {
  for (const target of revealTargets) {
    target.classList.add("is-visible");
  }
}

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
  status.dataset.updating = "true";
  status.textContent = text;
  status.dataset.kind = kind;
  window.setTimeout(() => {
    delete status.dataset.updating;
  }, 260);
}
