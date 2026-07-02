import "./styles.css";
import { inject } from "@vercel/analytics";

inject();

document.querySelector<HTMLDivElement>("#app")!.innerHTML = `
  <div class="page-shell">
    <header class="topbar">
      <div class="topbar-brand">
        <a class="mark" href="/" aria-label="Cull home">
          <img src="/images/cull-app-logo.png" alt="" />
          <span class="mark-name">CULL</span>
        </a>
        <span class="topbar-tagline">local-first &times; agent-ready: mcp &times; cli &times; skill</span>
      </div>
      <span class="release-note">
        <svg class="osi-mark" viewBox="0 0 24 24" role="img" aria-label="Open source" fill="currentColor"><path fill-rule="evenodd" d="M12 1.6C6.26 1.6 1.6 6.26 1.6 12c0 4.51 2.87 8.35 6.88 9.79l2.06-5.53a4.52 4.52 0 1 1 2.92 0l2.06 5.53A10.41 10.41 0 0 0 22.4 12c0-5.74-4.66-10.4-10.4-10.4Z"/></svg>
        open source
      </span>
    </header>

    <section class="hero" aria-labelledby="hero-title">
      <div class="hero-copy">
        <h1 id="hero-title" class="hero-step-2">
          <span class="h1-connector">Go from</span>
          <button class="rotating-line-control" type="button" data-rotating-line="from" aria-label="Change starting material">
            <span data-rotating-line-value="from">500 images</span>
          </button>
          <span class="h1-connector">to</span>
          <button class="rotating-line-control rotating-line-control--outcome" type="button" data-rotating-line="to" aria-label="Change finished outcome">
            <span data-rotating-line-value="to">20 keepers</span>
          </button>
        </h1>
        <p class="lede hero-step-3">A fast image review tool for people who shoot, generate, or produce at volume. Your files stay on your Mac.</p>
      </div>
      <figure class="product-shot hero-step-4">
        <img src="/images/cull-state-preview.png" alt="Loupe view with an image selected, an agent context menu offering Regenerate, Select, and Review, and a pending agent proposal in the side rail" />
      </figure>
      <aside class="download-block hero-step-5" aria-label="Download Cull">
        <div class="download-copy">
          <p class="download-title">Get Cull</p>
          <p class="download-specs">macOS 11+ &middot; Apple Silicon &middot; free &amp; open source</p>
        </div>
        <a class="download-button" href="https://github.com/glebis/cull/releases/latest" data-download-button>
          Download
          <span class="download-version">v0.2.4 &middot; .dmg</span>
        </a>
      </aside>
      <form class="signup-form signup-form--featured hero-step-6" data-signup-form>
        <label for="email">Stay up to date with releases.</label>
        <div class="signup-row">
          <input id="email" name="email" type="email" autocomplete="email" placeholder="you@example.com" aria-describedby="signup-status" required />
          <button type="submit" data-submit-button>Get notified</button>
        </div>
        <p id="signup-status" class="form-status" data-form-status aria-live="polite">One confirmation email. No lists, no noise.</p>
      </form>
    </section>

    <section class="feature-note feature-note--boring reveal-surface" aria-labelledby="boring-title" data-reveal>
      <figure class="feature-note-illustration reveal-item reveal-delay-0">
        <img src="/images/boring-work-generated.png" alt="Photos, artwork, contact sheets, and color swatches being sorted into a portfolio box" />
      </figure>
      <div class="feature-note-copy reveal-item reveal-delay-2">
        <h2 id="boring-title">The boring part of creative work, made fast</h2>
        <p>Most tools are built for editing. This is built for the moment before that, when you have hundreds of images and need to reach a final set.</p>
        <p>Look carefully, decide clearly, and make your work available to people and agents without friction.</p>
      </div>
    </section>

    <section class="feature-note feature-note--artist reveal-surface" aria-labelledby="artist-title" data-reveal>
      <figure class="feature-note-illustration reveal-item reveal-delay-0">
        <img class="artist-founder-image artist-founder-image--camera" src="/images/artist-founder-camera-generated.png" alt="" />
      </figure>
      <div class="feature-note-copy reveal-item reveal-delay-2">
        <h2 id="artist-title">Made by artists for artists</h2>
        <p>I, <a href="https://www.linkedin.com/in/glebkalinin/">Gleb Kalinin</a>, built this after getting tired of expensive, slow tools that made image work feel heavier than it needed to be. I wanted something <span class="founder-note-emphasis">open, local, and agent-friendly</span>: closer to Obsidian for images than another locked creative suite.</p>
      </div>
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

    <section class="feature-note feature-note--agent-sdk reveal-surface" aria-labelledby="agent-sdk-title" data-reveal>
      <figure class="feature-note-illustration reveal-item reveal-delay-0">
        <img src="/images/agent-sdk-generated.png" alt="A person at a desk while a small robot assistant sorts a stream of pictures into two neat stacks" loading="lazy" />
      </figure>
      <div class="feature-note-copy reveal-item reveal-delay-2">
        <p class="eyebrow">new / claude agent sdk</p>
        <h2 id="agent-sdk-title">Ask Claude to do the first pass</h2>
        <p>Cull now ships with a built-in agent chat powered by the Claude Agent SDK. Describe what you want in plain language — "pick the sharpest shot from every series", "shortlist the warm portraits" — and Claude works through your library.</p>
        <p>Nothing changes without you: the agent proposes a selection, you see exactly which images are affected and what it costs, then approve or reject. Prefer your own setup? The same surface is open to any agent over MCP or the headless CLI.</p>
      </div>
    </section>

    <section class="workflow reveal-surface" aria-labelledby="workflow-title" data-reveal>
      <div class="reveal-item reveal-delay-0">
        <h2 id="workflow-title">From folder to final set</h2>
      </div>
      <div class="workflow-list">
        <article class="reveal-item reveal-delay-1" data-command="import folder">
          <figure class="workflow-illustration"><img src="/images/workflow-folder.png" alt="" loading="lazy" /></figure>
          <h3>Drop in your folder</h3>
          <p>Drag in any folder, any size, any structure. The app reads everything and stays out of the way.</p>
        </article>
        <article class="reveal-item reveal-delay-2" data-command="open loupe">
          <figure class="workflow-illustration"><img src="/images/workflow-loupe.png" alt="" loading="lazy" /></figure>
          <h3>See every shot clearly</h3>
          <p>Grid, loupe, and side-by-side compare. Move through images at whatever pace works.</p>
        </article>
        <article class="reveal-item reveal-delay-3" data-command="find similar">
          <figure class="workflow-illustration"><img src="/images/workflow-search.png" alt="" loading="lazy" /></figure>
          <h3>Find what you are looking for</h3>
          <p>Search by look and feel rather than filename. Surface the sharp ones, the warm ones, or the ones that match a reference.</p>
        </article>
        <article class="reveal-item reveal-delay-4" data-command="export keepers">
          <figure class="workflow-illustration"><img src="/images/workflow-export.png" alt="" loading="lazy" /></figure>
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
        <h2 id="privacy-title">Be first when it ships</h2>
      </div>
      <div class="bottom-signup-copy reveal-item reveal-delay-2">
        <form class="signup-form signup-form--featured signup-form--bottom" data-signup-form>
          <label for="bottom-email">Stay up to date with releases.</label>
          <div class="signup-row">
            <input id="bottom-email" name="email" type="email" autocomplete="email" placeholder="you@example.com" aria-describedby="bottom-signup-status" required />
            <button type="submit" data-submit-button>Get notified</button>
          </div>
          <p id="bottom-signup-status" class="form-status" data-form-status aria-live="polite">One confirmation email. No lists, no noise.</p>
        </form>
      </div>
    </section>

    <figure class="footer-illustration reveal-surface" aria-label="Image workflow from local archive to agent surfaces" data-reveal>
      <img src="/images/footer-line-map.png" alt="" />
    </figure>

    <footer class="site-footer">
      <div class="site-footer-meta">
        <p>Made in 🇪🇺 Berlin by <a href="https://www.linkedin.com/in/glebkalinin/">Gleb Kalinin</a></p>
        <p class="footer-fineprint">No cookies, no cross-site tracking.</p>
      </div>
      <nav aria-label="Footer links">
        <a href="https://github.com/glebis/cull">Repository</a>
        <a href="https://github.com/glebis">Gleb's Github</a>
        <a href="https://t.me/glebkalinin">Telegram</a>
      </nav>
    </footer>
  </div>
`;

const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
const pageShell = document.querySelector<HTMLElement>(".page-shell");

if (pageShell) {
  protectHangingWords(pageShell);
}

if (motionQuery.matches) {
  document.documentElement.classList.add("reduced-motion");
}

const rotatingLines = {
  from: {
    index: 0,
    values: ["500 images", "a pile of sketches", "dusty SD cards", "751 test shots"],
  },
  to: {
    index: 0,
    values: ["20 keepers", "an exhibition", "a publication", "a movie", "an Insta post", "a photo album", "a documentary"],
  },
} satisfies Record<string, { index: number; values: string[] }>;

type RotatingLineName = keyof typeof rotatingLines;

let rotatingLineTurn: RotatingLineName = "from";
let rotatingLineTimer: number | undefined;

for (const button of document.querySelectorAll<HTMLButtonElement>("[data-rotating-line]")) {
  button.addEventListener("click", () => {
    cycleRotatingLine(button.dataset.rotatingLine, 1, true);
  });

  button.addEventListener("keydown", (event) => {
    if (event.key === "ArrowRight" || event.key === "ArrowUp") {
      event.preventDefault();
      cycleRotatingLine(button.dataset.rotatingLine, 1, true);
    }
    if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
      event.preventDefault();
      cycleRotatingLine(button.dataset.rotatingLine, -1, true);
    }
  });
}

startRotatingLineTimer();

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
  const messages: Record<string, { text: string; kind: StatusKind; note?: StatusNote }> = {
    confirmed: { text: "Confirmed. You are on the launch list.", kind: "success", note: "confirmed" },
    already_confirmed: { text: "Already confirmed. You are on the list.", kind: "success", note: "confirmed" },
    expired: { text: "That confirmation link expired. Submit your email again for a fresh link.", kind: "error" },
    invalid: { text: "That confirmation link is not valid. Submit your email again for a fresh link.", kind: "error" },
  };
  const message = messages[queryState];
  if (message) {
    setAllStatuses(message.text, message.kind, { note: message.note });
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
      const body = (await response.json()) as { message?: string; ok?: boolean; status?: string };
      setStatus(form, body.message ?? "Signup is temporarily unavailable.", body.ok ? "success" : "error", {
        note: body.ok ? statusNoteForSubscribeStatus(body.status) : undefined,
      });
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
type StatusNote = "confirmation-sent" | "confirmed";

function setAllStatuses(text: string, kind: StatusKind, options: { note?: StatusNote } = {}): void {
  for (const form of document.querySelectorAll<HTMLFormElement>("[data-signup-form]")) {
    setStatus(form, text, kind, options);
  }
}

function setStatus(form: HTMLFormElement, text: string, kind: StatusKind, options: { note?: StatusNote } = {}): void {
  const status = form.querySelector<HTMLElement>("[data-form-status]");
  if (!status) {
    return;
  }
  status.dataset.updating = "true";
  status.textContent = formatTextForLineBreaks(text);
  status.dataset.kind = kind;
  form.dataset.statusKind = kind;
  if (options.note) {
    form.dataset.statusNote = options.note;
  } else {
    delete form.dataset.statusNote;
  }
  window.setTimeout(() => {
    delete status.dataset.updating;
  }, 260);
}

function cycleRotatingLine(slotName: string | undefined, direction = 1, manual = false): void {
  if (!isRotatingLineName(slotName)) {
    return;
  }

  const slot = rotatingLines[slotName];
  slot.index = (slot.index + direction + slot.values.length) % slot.values.length;
  const value = document.querySelector<HTMLElement>(`[data-rotating-line-value="${slotName}"]`);
  const button = document.querySelector<HTMLElement>(`[data-rotating-line="${slotName}"]`);
  if (!value || !button) {
    return;
  }

  value.textContent = formatTextForLineBreaks(slot.values[slot.index]);
  button.classList.remove("is-swapping", "is-active");
  void button.offsetWidth;
  button.classList.add("is-swapping", "is-active");

  window.setTimeout(() => {
    button.classList.remove("is-active");
  }, 620);

  rotatingLineTurn = slotName === "from" ? "to" : "from";
  if (manual) {
    startRotatingLineTimer();
  }
}

function isRotatingLineName(slotName: string | undefined): slotName is RotatingLineName {
  return slotName === "from" || slotName === "to";
}

function startRotatingLineTimer(): void {
  if (motionQuery.matches) {
    return;
  }
  if (rotatingLineTimer !== undefined) {
    window.clearInterval(rotatingLineTimer);
  }
  rotatingLineTimer = window.setInterval(() => {
    cycleRotatingLine(rotatingLineTurn);
  }, 2400);
}

function statusNoteForSubscribeStatus(status: string | undefined): StatusNote | undefined {
  switch (status) {
    case "pending":
      return "confirmation-sent";
    case "already_confirmed":
      return "confirmed";
    default:
      return undefined;
  }
}

function protectHangingWords(root: HTMLElement): void {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement;
      if (!parent || !node.textContent?.trim()) {
        return NodeFilter.FILTER_REJECT;
      }
      if (["SCRIPT", "STYLE", "TEXTAREA"].includes(parent.tagName)) {
        return NodeFilter.FILTER_REJECT;
      }
      return NodeFilter.FILTER_ACCEPT;
    },
  });

  const textNodes: Text[] = [];
  while (walker.nextNode()) {
    textNodes.push(walker.currentNode as Text);
  }

  for (const node of textNodes) {
    node.textContent = formatTextForLineBreaks(node.textContent ?? "");
  }
}

function formatTextForLineBreaks(text: string): string {
  const shortWords = [
    "a",
    "an",
    "and",
    "as",
    "at",
    "by",
    "for",
    "from",
    "in",
    "is",
    "it",
    "no",
    "of",
    "on",
    "or",
    "the",
    "to",
    "via",
    "with",
  ];
  const shortWordPattern = new RegExp(`\\b(${shortWords.join("|")})\\s+`, "gi");

  return text
    .replace(shortWordPattern, "$1\u00a0")
    .replace(/\b(\d+)\s+(?=\S)/g, "$1\u00a0");
}
