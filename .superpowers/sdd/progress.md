# Release automation SDD progress

- Task 9: complete (`8f713c841..dae8d4288`). Release-specific suite 108/108,
  full repository suite 1,059/1,059, workflow lint and syntax checks green.
  Controller review found and corrected the legitimate in-progress publication
  race; independent reviewer threads were attempted but did not return a verdict.
- Task 10: complete. Protected-main CI run `29240012008` passed; signed
  non-publishing canary `29240223393` passed and was independently reverified
  locally; main ruleset `18866632` and immutable-tag ruleset `18866636` are
  active; `CULL_RELEASE_PUBLISH_ENABLED=true` was set only after those gates.
- Modular skills: complete in `glebis/claude-skills` PR 33. Six validated skills
  are symlinked from the canonical checkout into `~/.agents/skills/`.
