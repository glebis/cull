//! Install the `cull` CLI onto the user's PATH.
//!
//! The GUI app and the headless CLI are the same binary (see `cli/mod.rs`), which
//! normally lives inside the bundle at `Cull.app/Contents/MacOS/cull`. This module
//! creates a symlink to it from a directory that is already on the user's PATH, so
//! the `cull ...` invocations in the README and docs work as written.
//!
//! Homebrew cask installs get this for free via the cask's `binary` stanza; this
//! command exists for people who installed from the DMG.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Name of the command as it should appear on PATH.
pub const CLI_NAME: &str = "cull";

/// Directories we are willing to install into, most-preferred first.
///
/// Deliberately a fixed list rather than a split of `env::var("PATH")`: a GUI app
/// launched from Finder inherits launchd's environment, whose PATH is almost never
/// the user's shell PATH. `$HOME` is substituted for a leading `~`.
pub const CANDIDATE_DIRS: &[&str] = &[
    "/opt/homebrew/bin", // Apple Silicon Homebrew — on PATH for brew users, user-writable
    "/usr/local/bin",    // Intel Homebrew / classic; on PATH by default but often root-owned
    "~/.local/bin",      // XDG-style user bin; no privileges needed, may not be on PATH
    "~/bin",             // traditional; sourced by the default macOS zsh profile when present
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliToolStatus {
    /// A symlink named `cull` exists in one of the candidate directories.
    pub installed: bool,
    /// Where that symlink lives, if installed.
    pub link_path: Option<String>,
    /// Where it currently points.
    pub target_path: Option<String>,
    /// Installed, but pointing at a different binary than the running one
    /// (app moved, or a second copy of Cull installed). Offer a re-install.
    pub stale: bool,
    /// Directory `install_cli_tool` would use, if not installed.
    pub candidate_dir: Option<String>,
    /// Set when the chosen directory is not on the user's shell PATH; the UI shows
    /// this line for the user to add to their shell profile.
    pub path_hint: Option<String>,
}

/// Absolute, symlink-resolved path of the binary that is currently running.
///
/// Mirrors `lib.rs:178`, which uses `current_exe` to relaunch the same binary in
/// tray mode. Resolving is important here: if the app was itself launched through
/// a symlink we want to record the real bundle path, not the link.
fn running_binary() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate Cull's binary: {e}"))?;
    std::fs::canonicalize(&exe).map_err(|e| format!("Cannot resolve {}: {e}", exe.display()))
}

/// Expand a leading `~` against `$HOME`. Returns `None` if `$HOME` is unset.
fn expand_home(dir: &str) -> Option<PathBuf> {
    match dir.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(|home| PathBuf::from(home).join(rest)),
        None => Some(PathBuf::from(dir)),
    }
}

/// True when `dir` appears as a component of `path_env` (a `:`-separated PATH).
fn is_on_path(dir: &Path, path_env: &str) -> bool {
    path_env
        .split(':')
        .filter(|entry| !entry.is_empty())
        .any(|entry| expand_home(entry).is_some_and(|entry| entry == dir))
}

/// True when we can create files in `dir` without escalating privileges.
/// A directory that does not exist yet counts as writable if we could create it
/// (i.e. its parent is writable) — `install` will `create_dir_all` it.
fn is_user_writable(dir: &Path) -> bool {
    if dir.is_dir() {
        return !std::fs::metadata(dir)
            .map(|m| m.permissions().readonly())
            .unwrap_or(true);
    }
    match dir.parent() {
        Some(parent) => parent.is_dir() && is_user_writable(parent),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// TODO(you): implement the install-directory policy.
// ---------------------------------------------------------------------------

/// Choose the directory to symlink `cull` into.
///
/// `path_env` is the user's *shell* PATH (the caller obtains it by asking the login
/// shell, not from `env::var`). Every helper you need is above:
/// `CANDIDATE_DIRS`, `expand_home`, `is_on_path`, `is_user_writable`.
///
/// Return the chosen directory plus a `path_hint`: `Some(line)` when the directory
/// is NOT on `path_env` and the user must add it to their shell profile themselves,
/// `None` when it is already on PATH and the command will just work.
///
/// The trade-off to encode: preferring a directory that's already on PATH gives a
/// working `cull` with no follow-up, but those directories (`/usr/local/bin`) are
/// often root-owned, and we have no admin prompt. Preferring a writable directory
/// always succeeds but may leave the user with a symlink their shell can't see.
/// Decide which failure you would rather hand a first-time user.
fn resolve_install_dir(path_env: &str) -> Result<(PathBuf, Option<String>), String> {
    let _ = (path_env, CANDIDATE_DIRS);
    todo!("choose an install directory and build the PATH hint")
}

// ---------------------------------------------------------------------------

/// The user's login-shell PATH.
///
/// `env::var("PATH")` inside a Finder-launched app returns launchd's PATH, which
/// omits `/opt/homebrew/bin` and anything the user set in their profile. Asking the
/// login shell is the only way to see what `cull` would actually resolve against.
fn shell_path_env() -> String {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    std::process::Command::new(&shell)
        .args(["-lic", "printf %s \"$PATH\""])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| std::env::var("PATH").unwrap_or_default())
}

/// Find an existing `cull` symlink we installed, in any candidate directory.
fn existing_link() -> Option<(PathBuf, Option<PathBuf>)> {
    for dir in CANDIDATE_DIRS {
        let Some(link) = expand_home(dir).map(|dir| dir.join(CLI_NAME)) else {
            continue;
        };
        let Ok(meta) = std::fs::symlink_metadata(&link) else {
            continue;
        };
        if !meta.file_type().is_symlink() {
            // A real file named `cull` that we did not create (e.g. a Homebrew
            // cask binary, or another tool entirely). Report it, never replace it.
            return Some((link, None));
        }
        let target = std::fs::read_link(&link).ok();
        return Some((link, target));
    }
    None
}

#[tauri::command]
pub async fn cli_tool_status() -> Result<CliToolStatus, String> {
    let running = running_binary()?;
    let path_env = shell_path_env();

    if let Some((link, target)) = existing_link() {
        let resolved = target.as_ref().and_then(|t| std::fs::canonicalize(t).ok());
        let stale = resolved.as_deref() != Some(running.as_path());
        return Ok(CliToolStatus {
            installed: true,
            link_path: Some(link.display().to_string()),
            target_path: target.map(|t| t.display().to_string()),
            stale,
            candidate_dir: None,
            path_hint: None,
        });
    }

    let (dir, path_hint) = resolve_install_dir(&path_env)?;
    Ok(CliToolStatus {
        installed: false,
        link_path: None,
        target_path: None,
        stale: false,
        candidate_dir: Some(dir.display().to_string()),
        path_hint,
    })
}

#[tauri::command]
pub async fn install_cli_tool() -> Result<CliToolStatus, String> {
    #[cfg(not(unix))]
    {
        return Err("Installing the command line tool is only supported on macOS.".to_string());
    }

    #[cfg(unix)]
    {
        let running = running_binary()?;
        let path_env = shell_path_env();
        let (dir, path_hint) = resolve_install_dir(&path_env)?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;

        let link = dir.join(CLI_NAME);
        match std::fs::symlink_metadata(&link) {
            Ok(meta) if meta.file_type().is_symlink() => {
                // Ours (or a previous install pointing at a moved app) — replace it.
                std::fs::remove_file(&link)
                    .map_err(|e| format!("Cannot replace {}: {e}", link.display()))?;
            }
            Ok(_) => {
                return Err(format!(
                    "{} already exists and is not a symlink. Remove it first if you want Cull to manage it.",
                    link.display()
                ));
            }
            Err(_) => {}
        }

        std::os::unix::fs::symlink(&running, &link)
            .map_err(|e| format!("Cannot link {}: {e}", link.display()))?;

        Ok(CliToolStatus {
            installed: true,
            link_path: Some(link.display().to_string()),
            target_path: Some(running.display().to_string()),
            stale: false,
            candidate_dir: None,
            path_hint,
        })
    }
}

#[tauri::command]
pub async fn uninstall_cli_tool() -> Result<CliToolStatus, String> {
    match existing_link() {
        Some((link, Some(_))) => {
            std::fs::remove_file(&link)
                .map_err(|e| format!("Cannot remove {}: {e}", link.display()))?;
        }
        Some((link, None)) => {
            return Err(format!(
                "{} is not a symlink Cull created; leaving it alone.",
                link.display()
            ));
        }
        None => {}
    }
    cli_tool_status().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home_prefix_only() {
        std::env::set_var("HOME", "/Users/test");
        assert_eq!(
            expand_home("~/.local/bin"),
            Some(PathBuf::from("/Users/test/.local/bin"))
        );
        assert_eq!(
            expand_home("/usr/local/bin"),
            Some(PathBuf::from("/usr/local/bin"))
        );
    }

    #[test]
    fn detects_path_membership_through_tilde() {
        std::env::set_var("HOME", "/Users/test");
        let dir = PathBuf::from("/Users/test/.local/bin");
        assert!(is_on_path(&dir, "/usr/bin:~/.local/bin:/bin"));
        assert!(is_on_path(&dir, "/usr/bin:/Users/test/.local/bin"));
        assert!(!is_on_path(&dir, "/usr/bin:/bin"));
    }

    #[test]
    fn ignores_empty_path_entries() {
        assert!(!is_on_path(&PathBuf::from("/usr/local/bin"), "::"));
    }

    #[test]
    fn temp_dir_is_user_writable() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(is_user_writable(tmp.path()));
        // A not-yet-created child of a writable dir is installable.
        assert!(is_user_writable(&tmp.path().join("bin")));
    }
}
