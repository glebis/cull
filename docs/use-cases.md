# Cull Use Cases and Jobs to Be Done

## Primary: Real use cases by real people

This section focuses on actual user behaviors seen in image workflows (artists, reviewers, and teams managing large AI image sets).

### 1) AI artist who needs reliable iteration history
- **Person**: AI artist generating hundreds of variations per prompt during a production sprint.
- **Situation**: Wants to compare variants, keep the best outputs, and quickly return to context when a direction changes.
- **Job**
  - *Track visual results against generation context (model, provider, seed, settings).*
  - *Find earlier variants by source/time/prompt traits and mark high-value outputs.*
  - *Reuse proven prompts/criteria on subsequent runs.*
- **How Cull helps**
  - Imports images and sidecars with generation metadata.
  - Stores generation provenance (`provider`, `model`, `seed`, settings) with image entries.
  - Supports fast search, filtering, and smart collections for rating/selection workflows.
  - Keeps local history without losing local truth from source files and adjacent metadata.

### 2) Creative director running a campaign review loop
- **Person**: Creative lead reviewing 200+ concept images across style and brief variants.
- **Situation**: Needs quick triage, stakeholder feedback, and traceability for rounds of approvals.
- **Job**
  - *Separate strong candidates from rejects in minutes.*
  - *Apply repeatable categories (style, mood, technical quality, brand fit).*
  - *Capture selection rationale and keep it discoverable over time.*
- **How Cull helps**
  - Ratings/collections/selection records provide an auditable selection layer.
  - Metadata filters and deterministic source detection make provenance and context easy to filter.
  - The interface supports rapid curation and export-quality workflows.

### 3) Prompt engineer iterating across generators
- **Person**: Prompt engineer tuning outputs across Midjourney, SD, DALL·E, ComfyUI, and Gemini.
- **Situation**: Needs to determine what changed between outputs before proposing the next prompt refinement.
- **Job**
  - *Find comparable outputs generated with nearby parameters.*
  - *Compare decisions quickly by visual similarity and metadata slices.*
  - *Isolate generation runs and reproduce what produced a strong result.*
- **How Cull helps**
  - Sidecar ingestion and generation run linking persist run-level metadata.
  - Searchable fields plus deterministic parsing for provider/model/quality/seed improve comparability.
  - Folder/file/source metadata + CLIP-based search aid pattern discovery across generations.

### 4) Small studio managing rights and provenance
- **Person**: Producer or admin maintaining safe image use in client work.
- **Situation**: Must answer provenance questions (“where did this come from?”) before delivery.
- **Job**
  - *Quickly establish source evidence for each image (tool, creation stream, constraints).*
  - *Detect suspiciously mixed workflows from imports and exports.*
  - *Keep source metadata attached as images move between local catalogs and projects.*
- **How Cull helps**
  - Evidence-backed source detection combines metadata, filename patterns, and image chunk signals.
  - Library design preserves import lineage while leaving user-curated tags and selections intact.

### 5) Individual creator building a reusable visual reference library
- **Person**: Motion/3D artist or designer building a reusable bank of references and mood pieces.
- **Situation**: Needs to recall prior ideas quickly when starting new projects.
- **Job**
  - *Find “similar-looking” prior images fast.*
  - *Reuse earlier selections as input for new creative sessions.*
  - *Keep project-level and global references synchronized.*
- **How Cull helps**
  - Global project-aware selection model supports persistent “best-of” sets.
  - Embedding-based retrieval accelerates finding visual neighbors.
  - Keyboard-first workflows and strong grouping/filtering support fast recollection.

### 6) Freelance editor checking local batches before handoff
- **Person**: Freelancer preparing deliverables from mixed folders of source and generated assets.
- **Situation**: Must cleanly package selected outputs while keeping rejected images non-destructive.
- **Job**
  - *Prepare delivery-ready sets without destructive local edits.*
  - *Avoid losing originals while maintaining a curated export surface.*
  - *Retain local reasoning for what was selected and why.*
- **How Cull helps**
  - Non-destructive database-backed selection model keeps originals intact.
  - Export-oriented metadata and selection state help preserve rationale.

### 7) Remote editor replacement for Capture One-style tethering
- **Person**: Remote editor, art director, or retoucher reviewing captures in near real time.
- **Situation**: Needs to review incoming image streams and advise on next-step selection/adjustments without physically being on the capture machine.
- **Job**
  - *Open the local library stream from another device or browser instantly.*
  - *Make approvals/rejections and communicate decisions while the shoot or batch continues.*
  - *Keep review flow continuity even when local file transfer workflows are not practical.*
- **How Cull helps**
  - Built-in streaming browser exposes selected folders and assets via a shareable HTTP server.
  - Public sharing enables low-friction off-site review sessions for client or teammate approval.
  - Remote reviews remain synchronized with local ratings/selection metadata for a single source of truth.
- **Positioning note**
  - This is practical remote review replacement for tethered workflows, especially for review speed and decision throughput.

## Core: Proposed use cases and Jobs-to-Be-Done (JTBD)

These are product opportunities to expand and institutionalize where the program creates the strongest value. Each job is phrased in classic outcome form.

### JTBD 1: Make it effortless to find the right image in a growing AI backlog
- **When** I have many generated images from multiple tools and sessions,  
  **I want** to locate a specific visual direction quickly by metadata, prompt, or similarity,  
  **So that** I can resume work fast and avoid context loss.
- **Signals to watch**
  - Reduced time-to-first-relevant-image in user workflows.
  - Increased use of filters over manual scrolling.
  - More repeated finds of prior selected items during new sessions.

### JTBD 2: Make provenance confidence non-negotiable
- **When** an image is reused or delivered,  
  **I want** to know where it came from (model/provider/source signals and sidecar history),  
  **So that** I can defend reuse decisions and maintain production trust.
- **Signals to watch**
  - Percentage of assets with complete generation metadata.
  - Time to answer “who generated this and how?” during review checkpoints.
  - Fewer downstream disputes around model/source origin.

### JTBD 3: Make curation scalable without losing context
- **When** I select hundreds of images across a campaign,  
  **I want** fast, repeatable rules and collections,  
  **So that** the team can converge on a shortlist with less cognitive overhead.
- **Signals to watch**
  - Reduction in “selection drift” between sessions.
  - Reusability of named collections and smart filters.
  - Increase in team reuse of shared collection patterns.

### JTBD 4: Make local-first workflows resilient
- **When** I work offline or across unstable network conditions,  
  **I want** core management features to remain fully usable,  
  **So that** my work doesn’t depend on external APIs for everyday cataloging.
- **Signals to watch**
  - Consistent import/filter/select flow during offline scenarios.
  - High local data retention and low dependency on network for library operations.
  - Positive retention of metadata and tags across restarts.

### JTBD 5: Make future actions from selected work simple and traceable
- **When** an image is finalized for downstream use,  
  **I want** to move from curation to action (review, export, handoff, plugin actions) in one logical path,  
  **So that** there is a cleaner handoff from ideation to production.
- **Signals to watch**
  - Lower action steps between “found” and “used.”
  - Increased use of action pipelines and export workflows.
  - Lower manual copy/paste and path-tracking errors.

### JTBD 6: Replace local-only tether reviews with fast remote approval sessions
- **When** high-volume review is needed away from the capture station,  
  **I want** to share live browser-accessible image streams,  
  **So that** remote stakeholders can approve/reject and send direction updates quickly.
- **Signals to watch**
  - Time from image ingestion to remote viewer visibility.
  - Reduction in review iteration latency and manual file handoff loops.
  - Higher fraction of review actions completed in the shared stream versus post-export email or chat workflows.

## How to use this document

1. Validate each Real Use Case with interview notes or usage telemetry before treating it as a formal requirement.
2. Prioritize JTBD by impact and effort, then convert top jobs into short user stories with measurable outcomes.
3. Add one “Acceptance criteria” line per JTBD before any implementation planning.

Example acceptance for JTBD 1:  
*Given a library of 10,000 images, a user can filter by prompt source and source-model plus visual similarity and find a known target image in under 30 seconds.*
