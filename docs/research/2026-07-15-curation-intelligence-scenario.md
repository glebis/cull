# Curation Intelligence Scenario: Research and Implementation Plan

**Date:** 2026-07-15  
**Status:** Proposed; three product decisions remain open  
**Scenario:** An overnight batch of roughly 400 generated images is ingested, grouped, rapidly judged, reduced to a few keepers, published, and safely composted. Over time, Cull learns the curator's individual taste well enough to prioritize review without silently making irreversible decisions.

## Executive conclusion

Cull already contains most of the operational substrate: watched-folder intake, generation sidecars, ratings, accept/reject decisions, undo, embeddings, perceptual hashes, quality metrics, smart collections, similarity search, export, static-site packaging, and reviewed trash proposals.

It does **not** yet learn an individual's taste. Its current “Best of Group” scorer is a fixed heuristic, its two clustering systems disagree, embedding/group generation is manually orchestrated, Speed Review compares arbitrary adjacent pairs, smart collections cannot filter on predicted taste, and “Publish” stops at a static package/local preview.

The scenario should therefore be built around one deep **Curation Intelligence module**, not another standalone AI screen. The module should convert explicit judgments plus visual evidence into versioned, explainable, confidence-bearing curation proposals. Existing selection, smart-collection, proposal, export, publishing, and trash modules should remain the adapters that apply those proposals.

Recommended delivery size:

- **Useful MVP:** 4–6 engineer-weeks — automatic batch processing, coherent grouping, active review queue, conservative taste suggestions, keeper smart collection, and reviewed compost proposals.
- **Full scenario:** 7–10 engineer-weeks — calibrated personalization, snapshot publishing with delivery records, retention-based compost lifecycle, evaluation tooling, recovery, accessibility, and full quality gates.

These are implementation estimates for one engineer familiar with Cull, not calendar commitments. Model-download UX, platform Trash differences, or a real remote publishing destination can expand the range.

## What exists today

### Overnight intake

- Watched library roots ingest new and modified images through a debounced queue.
- Imports compute SHA-256 identity, perceptual hashes, source evidence, and adjacent generation sidecars.
- New images do **not** automatically receive embeddings, quality analysis, or persisted groups. Those are separate manual actions.
- Existing background jobs can report progress, but there is no single resumable “prepare this batch for review” job.

### Judgment capture

- Global 0–5 ratings and accept/reject/undecided decisions are persisted, undo-backed, and logged as session events.
- Grid and Loupe already support keyboard-first curation.
- Speed Review captures pairwise decisions, but it pairs adjacent images and forces one accept plus one reject. This is unsuitable training data unless the pair is deliberately selected and “both/neither/skip” are available.

### Similarity and grouping

- CLIP, DINOv2, Gemini, and other embedding providers are supported.
- Exact cosine similarity and pHash near-duplicate search exist.
- Persisted similarity groups use greedy thresholding. The algorithm is seed-order-sensitive, non-transitive, and quadratic across the selected embeddings.
- Embedding Explorer separately runs UMAP and k-means in a browser worker. Those transient clusters are not the persisted groups used by ranking and smart collections.
- The frontend exposes `generateSimilarityGroups`, but no production UI calls it. Best-of-Group instructs the user to generate groups in Embeddings even though that bridge is absent.

### Smart collections

- Live filters already cover rating, decision, source, metadata, quality, colour, and persisted similarity group.
- Built-in collections already gather explicit 5-star, 4-star+, accepted, rejected, and unrated images.
- There are no fields for predicted taste, prediction confidence, group role, curation state, or review priority.
- Visual-similarity filter variants exist in the type system but are rejected by SQL evaluation.

### Ranking, export, and publishing

- Best-of-Group uses fixed weights: rating 40%, decision 30%, quality 20%, and representativeness 10%. Winner overrides are generic settings and are not learned from.
- Export supports image files, social/web compositions, contact sheets, PDFs, and CSV delivery lists.
- Static publishing builds a portable site package, manifest, variants, QR code, instructions, and local preview.
- There is no publication record, retry state, remote deployment adapter, or single “keepers to published result” workflow. Headless export also does not evaluate a smart collection ID.

### Compost and deletion

- Reject is safe metadata; it does not touch source files.
- Batch OS Trash is reviewed, audited, partially recoverable, and reports per-file outcomes.
- There is no compost/quarantine state, retention window, provenance-preserving tombstone, or “reject all non-winners in these groups” proposal.
- Automatic permanent deletion would violate the scenario's trust requirements and should remain out of scope.

## Relevant research

### Learning taste from judgments

Pairwise human choices can train a reward/ranking model over frozen image representations. Pick-a-Pic trains a CLIP-based scorer from real user choices, and ImageReward shows that human judgments can train useful image reward models. Both are population-level results; they do not prove that a generic scorer represents one person's taste.

Personalized image-aesthetics research explicitly finds that models fit some people better than others. Cull must therefore measure each personal profile on held-out judgments and expose uncertainty. It should not claim to have “learned your eye” merely because training loss decreased.

DINOv2's official model card specifically supports nearest-neighbour retrieval and lightweight linear/logistic classifiers over frozen features. That makes a versioned linear head over existing local DINOv2 embeddings a strong first implementation: small, explainable, fast to retrain, and easy to discard if it does not beat a baseline.

Sources:

- [Pick-a-Pic: An Open Dataset of User Preferences for Text-to-Image Generation](https://arxiv.org/abs/2305.01569)
- [ImageReward: Learning and Evaluating Human Preferences for Text-to-Image Generation](https://arxiv.org/abs/2304.05977)
- [Correct for Whom? Subjectivity and Personalized Image Aesthetics Assessment](https://ojs.aaai.org/index.php/AAAI/article/view/26395)
- [DINOv2 paper](https://arxiv.org/abs/2304.07193)
- [Official DINOv2 model card](https://github.com/facebookresearch/dinov2/blob/main/MODEL_CARD.md)

### Representation and grouping

DINOv2 is the better default for fine visual similarity; CLIP remains useful for semantic direction and text search but its official materials warn about fine-grained and systematic limitations. Cull should keep these roles distinct rather than calling every vector relationship “similarity.”

Density-based grouping is a better conceptual fit than fixed-k clustering because generated batches contain uneven families and genuine outliers. HDBSCAN demonstrates hierarchical density-based clusters with noise points. For the first 400-image workflow, exact pairwise distance is acceptable; an ANN index such as Faiss is a later scaling concern, not an MVP dependency.

pHash is useful duplicate evidence, not authoritative identity. Robustness research shows perceptual hashes can be deliberately evaded. Cull should combine exact hash, pHash, and visual embeddings, and explain which evidence created a group.

Sources:

- [OpenAI: CLIP](https://openai.com/index/clip/)
- [Official CLIP model card](https://github.com/openai/clip/blob/main/model-card.md)
- [hdbscan: Hierarchical density based clustering](https://joss.theoj.org/papers/10.21105/joss.00205)
- [Meta: Faiss similarity search](https://engineering.fb.com/2017/03/29/data-infrastructure/faiss-a-library-for-efficient-similarity-search/)
- [USENIX: robustness limits of perceptual hashing](https://www.usenix.org/conference/usenixsecurity22/presentation/jain)

### Choosing the next images to rate

Asking only for the most uncertain images produces redundant questions when a batch contains many nearly identical generations. BADGE and BatchBALD both support selecting informative **and diverse** batches. Cull's review queue should combine model uncertainty, cluster coverage, outlier coverage, and recency; it should not simply sort by score nearest to 0.5.

Sources:

- [BADGE: Deep Batch Active Learning by Diverse, Uncertain Gradient Lower Bounds](https://openreview.net/forum?id=ryghZJBKPS)
- [BatchBALD: Efficient and Diverse Batch Acquisition](https://openreview.net/forum?id=Bkxyn4reLS)

### Safe compost and provenance

OS-native Trash semantics should remain the only destructive endpoint. Apple's recycle interface returns source-to-trash mappings, the Windows file-operation interface supports recycle/undo-aware operations, and the freedesktop Trash specification records original paths and unique trash identities. Cull's current filename-based macOS undo is weaker than these semantics and should be hardened before retention-based compost can be trusted.

Exports should preserve AI provenance where the format permits it. IPTC 2025.1 defines AI system/version, prompt, prompt-writer, and digital-source fields. C2PA provides signed, content-bound provenance, but production signing introduces key-management and identity requirements; it should be a later, explicit capability rather than implied by ordinary metadata export.

Sources:

- [Apple `NSWorkspace.recycle`](https://developer.apple.com/documentation/appkit/nsworkspace/recycle%28_%3Acompletionhandler%3A%29)
- [Microsoft `IFileOperation` flags](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifileoperation-setoperationflags)
- [freedesktop.org Trash Specification](https://specifications.freedesktop.org/trash/latest/)
- [IPTC Photo Metadata Standard 2025.1](https://www.iptc.org/std/photometadata/specification/IPTC-PhotoMetadata-2025.1.html)
- [C2PA specifications](https://spec.c2pa.org/specifications/)
- [Official C2PA Rust library](https://github.com/contentauth/c2pa-rs)

## Proposed user journey

1. Overnight generations arrive in a watched folder. Cull creates an **intake batch** and incrementally imports originals plus generation sidecars.
2. A resumable preparation job computes missing thumbnails, exact/pHash identity, quality metrics, and the configured local embedding. Failures remain visible and retryable.
3. Cull builds evidence-based visual families and marks outliers. It does not force every image into a cluster.
4. The morning screen says, for example: “400 arrived · 392 ready · 6 still processing · 2 failed · 54 visual families.”
5. Review begins with representative, diverse frames. The user may rate directly or compare deliberately chosen near-neighbours with **left / right / both / neither / skip**.
6. After each small block, Cull retrains the personal profile, reports whether held-out performance beats the non-personal baseline, and reprioritizes the remaining review queue.
7. High-confidence candidates appear in a live **Likely Keepers** smart collection; uncertain candidates remain in **Needs Your Eye**. Nothing is accepted or rejected merely because of a prediction.
8. Cull proposes one or more keepers per visual family with reasons: explicit rating, preference score, confidence, quality flags, and family coverage.
9. The user approves a final keeper set. Cull materializes a **snapshot collection** so later model changes cannot silently alter what is published.
10. Publishing creates a versioned publication record and hands the snapshot to the configured destination adapter.
11. Non-keepers become rejected or `compost_pending`. Moving files to OS Trash is a separate reviewed action, optionally after a retention period. Permanent deletion is never automatic.

## Target design

### Highest testing seam: Curation Intelligence

The preferred external seam is one module with three operations:

```text
refresh_profile(scope) -> ProfileSnapshot
curate(scope, policy) -> CurationProposal
record_outcome(proposal_id, corrections) -> LearningReceipt
```

`refresh_profile` builds a versioned personal model from eligible user judgments and reports evaluation/calibration. `curate` returns assessments, a diverse review queue, family winners, keeper candidates, and compost candidates without applying mutations. `record_outcome` stores what the user approved, rejected, or corrected so the next profile can learn from actual outcomes.

The implementation can contain private adapters for embeddings, grouping, quality evidence, and model storage. Callers should not learn those details. The existing action-proposal module remains the mutation seam for applying selection and Trash actions.

### Data additions

Add explicit, typed records rather than hiding learned state in generic settings:

- **Intake batches and item status:** source scope, discovered/imported/prepared/failed counts, timestamps, retry information.
- **Judgment events:** actor, image(s), rating/decision/pairwise choice, before/after values, context, and whether the judgment was explicit or an approved proposal.
- **Preference profiles:** owner, feature model/version, algorithm/version, training cutoff, metrics, confidence tier, created/activated timestamps.
- **Predictions:** profile version, image, score, calibrated probability/confidence, explanation contributions, computed timestamp.
- **Grouping runs and memberships:** scope, evidence model/version, parameters, representative/medoid, membership strength, outlier status.
- **Curation state:** explicit lifecycle separate from rating (`unreviewed`, `review`, `keeper`, `rejected`, `compost_pending`, `trashed`).
- **Publication records:** immutable source snapshot, destination adapter, status, attempt history, output URL/path, provenance manifest, timestamps.

Current `session_events` can seed historical judgments, but new writes should capture old and new values explicitly so reversals and overrides are unambiguous.

### Model policy

- Default feature: local DINOv2 embedding for visual preference; CLIP/text features may be optional explanatory inputs later.
- First learner: regularized per-user linear/ordinal ranker. Pairwise choices use a Bradley–Terry-style loss; star ratings and accept/reject provide ordinal/binary supervision.
- Cold start: explicit collections only. Do not show learned keeper claims until minimum class coverage and held-out evaluation are met.
- Confidence tiers: `insufficient`, `experimental`, `useful`, and `degraded`. The exact thresholds must be chosen from real Cull sessions, not invented from training accuracy.
- Evaluation: time-based or session-based holdout; compare against rating frequency, group-representative, and general-aesthetic baselines. Track ranking AUC/nDCG plus calibration, not only classification accuracy.
- Retraining: on demand after a judgment block, then opportunistically in the background. Every prediction is tied to an immutable profile version.
- Drift: recent sessions may receive higher weight, but users need a “reset profile” and “rebuild from all history” control.
- Explanations: show contributing evidence and uncertainty; never claim the embedding itself explains taste.

### Grouping policy

Use distinct concepts:

- **Exact duplicates:** SHA-256 identity.
- **Near duplicates:** pHash plus tight visual-distance evidence.
- **Visual families:** DINOv2 neighbourhood/density structure with possible outliers.
- **Semantic directions:** CLIP/text similarity, not interchangeable with visual families.
- **Projection clusters:** visualization only unless explicitly materialized from a versioned grouping run.

For the MVP, process the current intake batch with exact distances and deterministic graph/density logic. Store the run and its parameters. Do not use the current greedy seed-order algorithm as the long-term canonical grouping method. Add ANN only when measured library sizes make exact scoped computation too slow.

### Smart-collection additions

Add filter fields for:

- preference score and confidence;
- profile version or “active profile”;
- curation state;
- review priority;
- grouping run, family ID, member role, and outlier state;
- intake batch;
- publication status.

Ship presets such as **New Overnight**, **Needs Your Eye**, **Likely Keepers**, **Approved Keepers**, **Near-Duplicate Losers**, and **Compost Pending**. Learned collections must visually distinguish predictions from explicit user decisions.

### Approval seams

No model or agent may directly:

- set final keeper/accept decisions without a reviewed proposal;
- move files to Trash without a reviewed, itemized action;
- permanently delete source files;
- publish to an external destination without an explicit publish action and destination summary.

Rating suggestions and smart-collection membership are reversible metadata and may update automatically. External publication and file movement remain human approval seams.

## Phased implementation plan

### Phase 0 — Reconcile contracts and measure the baseline (2–3 days)

- Reconcile stale issue state: several curation/export tasks remain open in the tracked issue export although their commits and UI exist.
- Write scenario fixtures: 400-image batch, duplicate families, outliers, sidecars, ratings, overrides, failures, and three known keepers.
- Measure current import, embedding, grouping, review, export, and Trash behavior.
- Lock vocabulary and lifecycle states before schema work.

**Exit:** agreed scenario definition, test fixture, baseline timings, and issue graph with no duplicate work.

### Phase 1 — Automatic, resumable batch preparation (4–6 days)

- Add intake-batch persistence and item statuses.
- Chain import → thumbnail/pHash → quality → configured embedding as bounded background jobs.
- Reprocess only missing/stale derived data; include analyzer/model versions.
- Surface progress, partial failures, cancellation, retry, and safe restart.

**Exit:** dropping 400 files into a watched folder yields a visible batch that becomes review-ready without manual command orchestration.

### Phase 2 — One canonical grouping substrate (5–7 days)

- Separate duplicate, near-duplicate, visual-family, semantic, and projection concepts in types and UI.
- Replace or version the greedy grouping algorithm with deterministic scoped graph/density grouping and outlier handling.
- Persist grouping runs and evidence; wire Embedding Explorer and Best-of-Group to the same persisted run.
- Add an explicit Generate/Refresh Families control and incremental refresh after intake.
- Preserve pHash and embedding provenance in explanations.

**Exit:** the same family is shown consistently in Explorer, ranking, smart collections, and proposals; outliers remain unforced.

### Phase 3 — Personal preference profile and active review (7–10 days)

- Add typed judgment, profile, prediction, and evaluation storage.
- Train a local regularized ranker over frozen DINOv2 features.
- Convert ratings, decisions, deliberate pairwise choices, and winner overrides into weighted training examples.
- Redesign Speed Review around model-selected, family-aware comparisons with both/neither/skip.
- Generate a diverse review queue from uncertainty, family coverage, and outlier coverage.
- Show profile health, held-out performance, confidence tier, reasons, rebuild, and reset.

**Exit:** on a held-out fixture, the personal model beats fixed heuristics before suggestions are enabled; low-confidence profiles fall back to explicit sorting.

### Phase 4 — Keeper workflow and learned smart collections (5–7 days)

- Add preference, confidence, curation-state, family-role, and review-priority filters.
- Create prediction-aware presets with unmistakable “suggested” versus “you chose” states.
- Produce itemized curation proposals: keep, review, reject, and near-duplicate loser, each with reason and confidence.
- Reuse the existing proposal review and undo patterns to apply approved decisions.
- Let the user materialize approved keepers into an immutable manual snapshot.

**Exit:** a user can rate a small diverse subset, review suggested keepers, correct mistakes, and create a stable final set without scanning all 400 images.

### Phase 5 — Publish the snapshot (4–6 days for local/package; more for a remote target)

- Make smart-collection export evaluation first-class, but require publication to use a materialized snapshot.
- Add publication records, attempts, outputs, provenance manifests, and retry semantics.
- Deepen the existing Publish module into destination adapters: local package/local preview first; one remote adapter only after the target is chosen.
- Preserve generation metadata in manifests and IPTC-compatible exports where possible.
- Treat C2PA signing as a separate opt-in project with production key management.

**Exit:** an approved three-image snapshot produces a recorded, repeatable published result; later preference changes cannot alter it silently.

### Phase 6 — Compost lifecycle and hardening (5–7 days)

- Add `compost_pending` with configurable retention and clear counts.
- Generate “all rejected non-winners” proposals; protect keepers, published snapshots, manually exempted images, and unresolved family members.
- Harden platform Trash adapters to retain source-to-trash identity and robust restore semantics.
- Keep permanent deletion manual and outside automation.
- Add interruption, disk-full, collision, missing-volume, partial-publish, and partial-trash recovery tests.

**Exit:** the user can safely approve cleanup of the remainder, undo/recover supported operations, and audit exactly what happened.

## Testing decisions

Tests should cross the highest stable seam and assert external behaviour, not private model steps.

### Curation Intelligence contract tests

- Same inputs and versions produce deterministic proposals.
- No prediction appears without its profile version, confidence, and evidence.
- Insufficient/degraded profiles return a conservative fallback.
- Training excludes held-out judgments and future events.
- Corrections affect the next profile and never rewrite historical model versions.
- Review batches cover uncertainty and diversity rather than repeating one near-duplicate family.
- Published or protected images never appear in compost proposals.

### Database and migration tests

- Full-migrate realistic databases; do not hand-build partial schemas.
- Preserve all existing selections, events, groups, collections, publication assets, and user files.
- Test migration interruption and schema invariants.
- Run the whole Rust library suite after shared DB/open/query changes.

### UI and E2E tests

- 400-image intake progress, restart, retry, and partial failure.
- Keyboard-only review including both/neither/skip and undo.
- Suggested versus explicit state is perceivable and accessible.
- Family identity is consistent across Explorer, review, smart collections, and proposal dialogs.
- Snapshot → publish → recorded output.
- Compost proposal review, protected-image guards, partial Trash failure, and undo.

### Product evaluation

- Time from batch ready to approved keeper snapshot.
- Number/fraction of images explicitly reviewed.
- Recall of user's held-out keepers and false-positive rate among suggested keepers.
- Calibration: suggestions at a claimed confidence should be correct at roughly that rate.
- Correction rate by family, generator, and session to detect drift or systematic bias.
- Zero unapproved external publications, Trash moves, or permanent deletions.

## Risks and pushback

- **Three good images cannot be guaranteed from visual similarity.** The system can prioritize and diversify review; it cannot infer an unstated brief or promise the “right” three.
- **A few ratings do not equal a stable personal model.** Cold-start and confidence gates are essential. The product should say “learning” and “suggested,” not “knows your taste.”
- **Arbitrary pairwise review is biased.** Current Speed Review should not be used as learning data without deliberate pair selection and both/neither outcomes.
- **One global taste profile may be wrong.** Preferences can differ by project, client, medium, or creative goal. Profile scope is a product decision.
- **Dynamic smart collections are unsafe publication sources.** Publishing must freeze a snapshot.
- **“Compost” must not mean silent deletion.** Rejection, quarantine, OS Trash, and permanent deletion are different lifecycle states.
- **Cloud publishing expands security and support scope.** Credentials, retries, rate limits, API changes, and deletion/replacement semantics are destination-specific.

## Decisions required from the product owner

1. **Profile scope:** one global personal eye, separate profile per project/session, or a global base with project-specific adaptation?  
   **Recommendation:** global base plus optional project profile. It can reuse history while preventing a client campaign from permanently redefining personal taste.

2. **Meaning of published:** is a static package/local preview sufficient, or must v1 deploy to a specific remote destination?  
   **Recommendation:** make the first vertical slice end at a versioned static package and publication record, then add exactly one chosen remote adapter.

3. **Meaning of compost:** rejected-and-hidden only, retention-based OS Trash, or immediate reviewed OS Trash?  
   **Recommendation:** reject immediately, default-hide, hold in `compost_pending` for 7 days, then require one reviewed batch move to OS Trash. Never automate permanent deletion.

## Suggested first vertical slice

Build Phases 0–4 against one watched folder and local DINOv2, then reuse the existing static package as the publish endpoint and keep compost non-destructive. This proves the product's unique claim—Cull learns enough from a small, diverse review set to surface likely keepers—before spending time on remote deployment or retention automation.

The slice is successful when a real 400-image batch can be prepared automatically, the user can explicitly review materially fewer than 400 images, Cull surfaces the actual keepers with measured confidence, corrections improve the next proposal, and an immutable keeper snapshot can be exported without any unapproved destructive action.
