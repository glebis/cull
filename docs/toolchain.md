# Toolchain setup

Cull pins the development toolchain so local machines and CI use the same major
runtime/compiler versions.

## Required versions

| Tool | Version | Pin file |
| --- | --- | --- |
| Node.js | 20.20.2 | `.node-version`, `.nvmrc` |
| npm | Use the npm bundled with Node.js 20.20.2 | Node.js distribution |
| Rust | 1.89.0 | `rust-toolchain.toml` |

Use your preferred version manager to install the pinned versions before running
checks:

```bash
nvm use
rustup toolchain install
npm ci
```

`rustup` reads `rust-toolchain.toml` automatically when commands are run from
this repository. Node version managers that support `.node-version` or `.nvmrc`
will select Node.js 20.20.2 automatically.

## `bd` binary selection

`bd` (beads) is the issue tracker of record. Some developer machines can have
multiple `bd` binaries on `PATH`, commonly both `/usr/local/bin/bd` and
`/opt/homebrew/bin/bd`, which makes it unclear which database client is being
used.

Use the repository wrapper instead of calling `bd` directly:

```bash
npm run bd -- ready
npm run bd -- show imageview-2w6.8
```

The wrapper resolves the binary deterministically:

1. `BD_BIN`, when set, must point to an executable and is used explicitly.
2. `/opt/homebrew/bin/bd`, when present, is preferred for Homebrew Apple Silicon
   installs.
3. `/usr/local/bin/bd`, when present, is used for Intel/Homebrew or manual
   installs.
4. Otherwise the first executable `bd` found on `PATH` is used.

The wrapper prints the selected binary before executing it so logs show which
client touched the beads database. To override the selection for one command:

```bash
BD_BIN=/usr/local/bin/bd npm run bd -- ready
```

## Dependency update automation

Dependabot is configured in `.github/dependabot.yml` to open weekly update pull
requests for:

- npm dependencies from `package.json` / `package-lock.json`
- Cargo dependencies from `src-tauri/Cargo.toml` / `src-tauri/Cargo.lock`
- GitHub Actions used by workflows in `.github/workflows/`

Review dependency update pull requests with the normal checks (`npm run ci`) and
run `npm run audit:licenses` whenever dependencies or model download policy
change.
