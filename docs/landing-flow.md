# Landing Flow

Use this flow when a feature branch is ready to move into `main` and become part
of the next main CI build:

```bash
npm run land:feature -- <feature-branch>
```

The script requires a clean worktree and the GitHub CLI. It prints the commit
list and changed-file summary for scope review, runs the full local preflight,
pushes only the feature branch, creates or updates its pull request, verifies
that the pull request still points at the exact feature commit, and waits for
every required check reported by GitHub. It fails closed if no required checks
are configured or visible.

After the required checks pass, GitHub merges the pull request using the exact
verified head commit. Only after GitHub confirms the merge does the script
switch to the local target branch and fast-forward it from `origin`. The script
does not merge into or push `main` locally, and removes the merged remote
feature branch.

Important distinction: main CI is not the signed release build. The main CI
workflow runs on pushes to `main`; the Release workflow is tag/manual triggered
and creates the packaged app artifacts.

Typical sequence:

1. Finish and commit a focused feature branch.
2. Run `npm run land:feature -- feature/name` from a clean worktree.
3. The script creates or updates the pull request, waits for required checks,
   merges it through GitHub, and fast-forwards local `main`.
4. Trigger the Release workflow separately when a tag/manual release build is
   needed.

Before release preparation, refresh `origin/main` and run
`npm run release:cull -- check --bump <patch|minor|major> --json`. The check and
prepare commands both run the blocking named behavior regression gate. A stale
hotfix line or release worktree cannot package successfully when it omits a
commit already present on verified `origin/main`; do not bypass this gate with a
manual tag.
