# Copyright of AI-Generated Code: Research Findings

*Researched 2026-05-12. Working research notes for Cull's AI-assisted development process.*

> Status: historical research notes, not a legal conclusion. For the current
> release checklist and license audit, use
> [OPEN_SOURCE_AUDIT.md](OPEN_SOURCE_AUDIT.md).

## TL;DR

Your workflow — specs before code, architectural decisions, iterative review, design system constraints — puts you in the "AI-assisted human-authored work" category, not "AI-generated output." The copyright position is defensible in both US and German jurisdictions, but requires documentation.

## 1. USCO Position (US Copyright Office)

The USCO's January 2025 Part 2 report states that **prompting alone does not qualify as human authorship** — prompts are "instructions, not creative expressions."

Copyright CAN protect AI-assisted works when a human:
- **Selects and arranges** AI outputs creatively
- **Edits or modifies** AI output adding original expression
- **Incorporates** AI elements into a larger human-authored work

Your workflow goes beyond prompting: you write architectural specs (AGENTS.md, design docs), design data models and interfaces, choose the stack, define the design system (CLAUDE.md), make UX decisions, review/reject/iterate on output, and arrange components into a coherent product.

The USCO would likely view this as a **human-authored work with AI-generated elements** — copyrightable in the human-contributed aspects.

## 2. German Courts: "Personality Reflected in Output" Test

Three 2026 German rulings set a high but achievable bar. Copyright is "conceivable" when the human exerts "creative influence on the design of the concrete work itself, for example through sufficiently individual presets."

Key test: **does the human's personality manifest in the output?**

Your architectural decisions (Tauri+Rust+SvelteKit+ONNX, Tokyo Night theme, specific component hierarchy, 23-tool MCP server design) are exactly these "sufficiently individual presets." A different developer prompting the same AI would produce a fundamentally different product.

**Burden of proof**: If challenged, you must "detail the human creative process." Session logs, design docs, CLAUDE.md constraints, and git history showing iterative refinement are precisely this evidence.

## 3. Software-Specific Considerations

A 2026 Carlton Fields analysis notes that for agentic coding workflows, **"architectural and orchestration artifacts may be the new moat that drives protectability."** Copyright may attach to "creative selection, coordination, or arrangement of AI-generated code blocks."

Software copyright protects *expression*, not *function*. Architecture, API design, component relationships, and UX flow are expressive choices. The syntax being AI-generated matters less than the system being human-designed.

## 4. Sweat of the Brow Won't Help

Neither the US nor the EU recognizes pure effort as a basis for copyright. The EU Software Directive requires a "natural person who created the program." Don't rely on "I worked hard prompting" — rely on **"I made creative decisions that shaped the output."**

## 5. What You Already Have (Strong Evidence)

- `AGENTS.md`, `CLAUDE.md` — human-authored constraints that shaped all code
- Design docs in `docs/superpowers/specs/` — architectural decisions predating code
- Git history showing iterative human review and refinement
- Claude Code session transcripts — full prompting record

## 6. What You Should Add

- **Architecture Decision Records (ADRs)** — brief docs explaining *why* specific approaches were chosen
- **Copyright notice in source files**: "Architecture and design by Gleb Kalinin. Implementation assisted by Claude (Anthropic)."
- **AUTHORSHIP file** documenting the human/AI split
- **Keep Claude Code session logs** — contemporaneous evidence of creative direction
- **Export key design decisions** from sessions where AI suggestions were rejected

## 7. Implications Per Licensing Scenario

- **Open source (Apache-2.0)**: Copyright claim covers the architecture/design layer. Anyone can copy purely AI-generated syntax, but the creative arrangement is yours.
- **Closed/paid**: Stronger copyright matters more. Document creative contribution aggressively.
- **App Store**: Apple doesn't prohibit AI-generated apps. A competitor copying your code could argue AI-generated portions are uncopyrightable — your architecture docs are defense.

## 8. Key Risk: Training Data Contamination

Biggest practical risk isn't copyright ownership — it's inadvertent GPL contamination from training data. Run `scancode-toolkit` or `fossa` before releasing.

## 9. Anthropic's ToS

Anthropic's Commercial ToS assign all right, title, and interest in outputs to the customer. They offer copyright indemnification and promise not to train on your inputs/outputs (commercial tier).

## Sources

- USCO AI Copyright Report Part 2 (Roth Jackson, 2025)
- USCO Copyrightability Analysis (Jones Day, 2025)
- Three German Rulings on AI Authorship (Bird & Bird, 2026)
- Munich Court on Prompt-Engineered Logos (2026)
- AI Code Copyright Protection (Carlton Fields / Bloomberg Law, 2026)
- AI Coding Protection (Bird & Bird Belgium, 2026)
- EU Copyright of AI-Generated Works (European Parliament, 2025)
- USCO AI Policy Guidance PDF
