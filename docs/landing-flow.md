# Landing Flow

Use this flow when a feature branch is ready to move into `main` and become part
of the next main CI build:

```bash
npm run land:feature -- <feature-branch>
```

The script requires a clean worktree, fetches the feature branch and `main`,
fast-forwards `main`, merges the feature branch with `--no-ff`, runs local
frontend checks, tests, and `npm run build`, syncs bd when supported or falls
back to `bd vc status`, pushes `main`, then watches main CI through GitHub.

Important distinction: main CI is not the signed release build. The main CI
workflow runs on pushes to `main`; the Release workflow is tag/manual triggered
and creates the packaged app artifacts.

Typical sequence:

1. Finish and push a feature branch.
2. Run `npm run land:feature -- feature/name` from a clean worktree.
3. Wait for main CI to pass.
4. Trigger the Release workflow separately when a tag/manual release build is
   needed.
