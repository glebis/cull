import "./styles.css";
import { inject } from "@vercel/analytics";

inject();

document.querySelector<HTMLDivElement>("#app")!.innerHTML = `
  <div class="page-shell">
    <header class="topbar">
      <a class="mark" href="/" aria-label="Cull home">
        <img src="/images/cull-app-logo.png" alt="" />
        <span class="mark-name">CULL</span>
      </a>
      <span class="topbar-tagline">local-first <span class="tagline-x">&times;</span> agent-ready: <span class="tagline-tag tagline-tag--mcp">mcp</span> <span class="tagline-x">&times;</span> <span class="tagline-tag tagline-tag--cli">cli</span> <span class="tagline-x">&times;</span> <span class="tagline-tag tagline-tag--skill">skill</span></span>
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
        <img src="/images/cull-grid-real.png" alt="Cull's grid view with smart collections in the sidebar and a library of images with ratings" />
      </figure>
      <aside class="download-block hero-step-5" aria-label="Download Cull">
        <div class="download-header">
          <img class="download-app-icon" src="/images/cull-app-logo.png" alt="" />
          <div class="download-copy">
            <p class="download-title">Download Cull</p>
            <p class="download-subtitle">Local-first image review, ready for you and your agents.</p>
          </div>
        </div>
        <a class="download-button" href="https://github.com/glebis/cull/releases/latest" data-download-button>
          <svg class="apple-mark" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M16.98 12.62c-.03-2.5 2.04-3.7 2.13-3.76-1.16-1.7-2.97-1.93-3.61-1.96-1.54-.16-3 .9-3.78.9-.77 0-1.98-.88-3.25-.86-1.67.03-3.21.97-4.07 2.47-1.74 3.01-.44 7.46 1.25 9.9.83 1.2 1.81 2.54 3.1 2.49 1.25-.05 1.72-.8 3.22-.8 1.5 0 1.93.8 3.25.78 1.34-.03 2.19-1.21 3-2.42a10.8 10.8 0 0 0 1.36-2.79c-.03-.02-2.6-1-2.6-3.95ZM14.5 5.27c.68-.83 1.15-1.98 1.02-3.12-.99.04-2.18.66-2.89 1.48-.63.73-1.19 1.9-1.04 3.02 1.1.09 2.22-.56 2.91-1.38Z"/></svg>
          Download for macOS
        </a>
        <ul class="download-specs-list">
          <li><span class="spec-icon" aria-hidden="true">&#9673;</span>macOS 11+ &middot; Apple Silicon</li>
          <li><span class="spec-icon" aria-hidden="true">&#9098;</span>v0.2.4 &middot; free &amp; open source</li>
        </ul>
        <p class="brew-label">or install with Homebrew</p>
        <div class="brew-row">
          <code class="brew-command" id="brew-command">brew install --cask glebis/tap/cull</code>
          <button class="brew-copy" type="button" data-brew-copy aria-label="Copy brew install command">Copy</button>
        </div>
      </aside>
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
        <h2>Agents when you want them</h2>
        <p>Integrate via MCP or CLI. Let agents help you cull, tag, and organize &mdash; on your terms.</p>
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
      <p class="eyebrow reveal-item reveal-delay-0" id="workflow-title">a faster review workflow</p>
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

    <section class="artist-panel reveal-surface" aria-labelledby="artist-title" data-reveal>
      <div class="artist-panel-copy reveal-item reveal-delay-1">
        <h2 id="artist-title">Made by artists for artists</h2>
        <p>I, <a href="https://www.linkedin.com/in/glebkalinin/">Gleb Kalinin</a>, built this after getting tired of expensive, slow tools that made image work feel heavier than it needed to be. I wanted something <span class="founder-note-emphasis">open, local, and agent-friendly</span>: closer to Obsidian for images than another locked creative suite.</p>
        <p>Free and open source. The code is public, your files stay on your machine, and no company can pull the tool away from you.</p>
        <p class="artist-panel-links">
          <a href="https://github.com/glebis/cull">GitHub</a><span class="tagline-x">|</span><a href="https://t.me/glebkalinin">Telegram</a><span class="tagline-x">|</span><a href="https://www.linkedin.com/in/glebkalinin/">LinkedIn</a>
        </p>
      </div>
      <figure class="artist-panel-illustration reveal-item reveal-delay-2">
        <img class="artist-founder-image artist-founder-image--camera" src="/images/artist-founder-camera-generated.png" alt="" />
      </figure>
    </section>

    <section class="pull-quote reveal-surface" aria-label="Keep what matters" data-reveal>
      <p class="pull-quote-text reveal-item reveal-delay-1">Keep what matters.</p>
    </section>

    <section class="bottom-signup reveal-surface" aria-label="Sign up for release updates" data-reveal>
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

    <footer class="site-footer">
      <div class="site-footer-brand">
        <img src="/images/cull-app-logo.png" alt="" />
      </div>
      <nav class="site-footer-columns" aria-label="Footer links">
        <div class="footer-col">
          <p class="footer-col-title footer-col-title--product">product</p>
          <a href="https://github.com/glebis/cull/releases/latest">Download</a>
          <a href="https://github.com/glebis/cull/blob/main/CHANGELOG.md">Changelog</a>
        </div>
        <div class="footer-col">
          <p class="footer-col-title footer-col-title--open">open source</p>
          <a href="https://github.com/glebis/cull">Repository</a>
          <a href="https://github.com/glebis/cull/issues">Issues</a>
          <a href="https://github.com/glebis/cull/blob/main/LICENSE">License</a>
        </div>
        <div class="footer-col">
          <p class="footer-col-title footer-col-title--community">community</p>
          <a href="https://github.com/glebis">Gleb's GitHub</a>
          <a href="https://t.me/glebkalinin">Telegram</a>
          <a href="https://www.linkedin.com/in/glebkalinin/">LinkedIn</a>
        </div>
      </nav>
      <a class="footer-star-card" href="https://github.com/glebis/cull">
        <span class="footer-star-title">&#9733; Star on GitHub</span>
        <span class="footer-star-copy">Help the project grow. Every star counts.</span>
      </a>
      <div class="site-footer-bottom">
        <p>Made in 🇪🇺 Berlin by <a href="https://www.linkedin.com/in/glebkalinin/">Gleb Kalinin</a> &middot; No cookies, no cross-site tracking.</p>
        <a href="https://github.com/glebis/cull/blob/main/LICENSE">MIT License</a>
      </div>
    </footer>
  </div>
`;

const brewCopyButton = document.querySelector<HTMLButtonElement>("[data-brew-copy]");
brewCopyButton?.addEventListener("click", async () => {
  const command = document.getElementById("brew-command")?.textContent?.replace(/ /g, " ").trim();
  if (!command) {
    return;
  }
  try {
    await navigator.clipboard.writeText(command);
    brewCopyButton.textContent = "Copied";
  } catch {
    brewCopyButton.textContent = "Select & copy";
  }
  window.setTimeout(() => {
    brewCopyButton.textContent = "Copy";
  }, 1800);
});

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
      if (["SCRIPT", "STYLE", "TEXTAREA", "CODE"].includes(parent.tagName)) {
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
