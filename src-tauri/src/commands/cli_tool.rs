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

/// Unique-enough name for the write probe, so parallel checks never collide.
fn write_probe_name() -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!(".cull-write-probe-{}-{}-{}", std::process::id(), nanos, seq)
}

/// True when a file can be created inside this exact directory right now.
///
/// Permission bits alone say nothing about the *current* user: a root-owned
/// `/usr/local/bin` with mode 755 reports `readonly() == false` yet rejects our
/// writes. Actually creating and removing a uniquely named probe file is the
/// honest check — it also covers ACLs and read-only volumes. A failed cleanup
/// reports the directory as unusable rather than claiming installation can succeed.
fn dir_accepts_writes(dir: &Path) -> bool {
    let probe = dir.join(write_probe_name());
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(file) => {
            drop(file);
            std::fs::remove_file(&probe).is_ok()
        }
        Err(_) => false,
    }
}

/// True when we can create files in `dir` without escalating privileges.
/// A directory that does not exist yet counts as writable if we could create it
/// (i.e. its parent is writable) — `install` will `create_dir_all` it.
fn is_user_writable(dir: &Path) -> bool {
    if dir.is_dir() {
        return dir_accepts_writes(dir);
    }
    match dir.parent() {
        Some(parent) => parent.is_dir() && is_user_writable(parent),
        None => false,
    }
}

/// One candidate directory plus the facts the policy needs. Gathered up front
/// (the only impure step) so `choose_candidate` stays pure and unit-testable.
struct Candidate {
    dir: PathBuf,
    /// Directory appears on the user's shell PATH.
    on_path: bool,
    /// The current user can create files in this directory (or create it).
    writable: bool,
    /// A real file named `cull` already exists here — something we did not
    /// install and must not overwrite. `install` would refuse, so prefer
    /// another directory when one exists.
    blocked: bool,
}

/// Filesystem facts for every candidate directory, in preference order.
fn collect_candidates(path_env: &str) -> Vec<Candidate> {
    CANDIDATE_DIRS
        .iter()
        .filter_map(|dir| expand_home(dir))
        .map(|dir| Candidate {
            on_path: is_on_path(&dir, path_env),
            writable: is_user_writable(&dir),
            blocked: matches!(inspect_entry(&dir.join(CLI_NAME)), ExistingEntry::Foreign),
            dir,
        })
        .collect()
}

/// Pick the install directory deterministically:
///
/// 1. the first candidate that is on the shell PATH, writable, and unoccupied —
///    a working `cull` with no follow-up, no admin prompt needed;
/// 2. the first writable, unoccupied candidate — succeeds without privileges,
///    and `path_hint_for` hands the user a paste-ready PATH line, an explained
///    and solvable follow-up;
/// 3. the first writable candidate at all — `install` reports the occupied
///    path with instructions instead of silently overwriting it.
fn choose_candidate(candidates: &[Candidate]) -> Option<&Candidate> {
    candidates
        .iter()
        .find(|c| c.on_path && c.writable && !c.blocked)
        .or_else(|| candidates.iter().find(|c| c.writable && !c.blocked))
        .or_else(|| candidates.iter().find(|c| c.writable))
}

/// Quote a value for POSIX shells so paths with spaces or quotes survive being
/// pasted into a profile (single-quoted, embedded quotes escaped).
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// The paste-ready profile line for a chosen directory, or `None` when it is
/// already on the shell PATH and the command will just work.
fn path_hint_for(dir: &Path, path_env: &str) -> Option<String> {
    if is_on_path(dir, path_env) {
        return None;
    }
    Some(format!(
        "export PATH={}:$PATH",
        shell_quote(&dir.display().to_string())
    ))
}

/// Choose the directory to symlink `cull` into.
///
/// `path_env` is the user's *shell* PATH (the caller obtains it by asking the login
/// shell, not from `env::var`).
fn resolve_install_dir(path_env: &str) -> Result<(PathBuf, Option<String>), String> {
    let candidates = collect_candidates(path_env);
    let chosen = choose_candidate(&candidates).ok_or_else(|| {
        "No writable folder is available for the cull command. Create ~/.local/bin, then try again."
            .to_string()
    })?;
    let dir = chosen.dir.clone();
    let path_hint = path_hint_for(&dir, path_env);
    Ok((dir, path_hint))
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

/// True when a link target is a Cull app bundle binary: any
/// `<…>/Cull.app/Contents/MacOS/cull` layout. Matched on the link's own target
/// string first (works for old/moved app locations and deleted bundles), then
/// on the fully resolved path (covers Homebrew cask indirection).
fn is_cull_bundle_link(target: &Path) -> bool {
    fn has_bundle_layout(path: &Path) -> bool {
        let mut components = path.components().rev();
        ["cull", "MacOS", "Contents", "Cull.app"]
            .iter()
            .all(|name| {
                components.next() == Some(std::path::Component::Normal(std::ffi::OsStr::new(name)))
            })
    }
    if has_bundle_layout(target) {
        return true;
    }
    std::fs::canonicalize(target)
        .map(|resolved| has_bundle_layout(&resolved))
        .unwrap_or(false)
}

/// How an existing `cull` entry in a candidate directory relates to Cull.
enum ExistingEntry {
    /// No entry.
    Absent,
    /// Symlink into a Cull.app bundle (current, moved, or Homebrew cask) —
    /// Cull may replace it during install or remove it during uninstall.
    ManagedLink(PathBuf),
    /// A real file, or a symlink pointing somewhere else. Never touched.
    Foreign,
}

fn inspect_entry(link: &Path) -> ExistingEntry {
    let Ok(meta) = std::fs::symlink_metadata(link) else {
        return ExistingEntry::Absent;
    };
    if meta.file_type().is_symlink() {
        if let Ok(target) = std::fs::read_link(link) {
            if is_cull_bundle_link(&target) {
                return ExistingEntry::ManagedLink(target);
            }
        }
        return ExistingEntry::Foreign;
    }
    ExistingEntry::Foreign
}

/// The first Cull-managed link (candidate order) plus every foreign `cull`
/// entry. Foreign entries are reported for conflict errors but are never
/// replaced or removed.
fn scan_existing_entries() -> (Option<(PathBuf, PathBuf)>, Vec<PathBuf>) {
    scan_existing_entries_in(
        &CANDIDATE_DIRS
            .iter()
            .filter_map(|dir| expand_home(dir))
            .collect::<Vec<PathBuf>>(),
    )
}

fn scan_existing_entries_in(dirs: &[PathBuf]) -> (Option<(PathBuf, PathBuf)>, Vec<PathBuf>) {
    let mut managed: Option<(PathBuf, PathBuf)> = None;
    let mut foreign: Vec<PathBuf> = Vec::new();
    for dir in dirs {
        let link = dir.join(CLI_NAME);
        match inspect_entry(&link) {
            ExistingEntry::Absent => {}
            ExistingEntry::ManagedLink(target) => {
                if managed.is_none() {
                    managed = Some((link, target));
                }
            }
            ExistingEntry::Foreign => foreign.push(link),
        }
    }
    (managed, foreign)
}

/// Actionable refusal for a `cull` entry Cull does not own.
#[cfg(unix)]
fn foreign_entry_error(link: &Path) -> String {
    match std::fs::read_link(link) {
        Ok(target) => format!(
            "{} points at {}, which is not a Cull app bundle. Cull will not replace it; remove that link yourself if you want Cull to manage it.",
            link.display(),
            target.display()
        ),
        Err(_) => format!(
            "{} already exists and is not a symlink. Remove it first if you want Cull to manage it.",
            link.display()
        ),
    }
}

#[tauri::command]
pub async fn cli_tool_status() -> Result<CliToolStatus, String> {
    let running = running_binary()?;
    let path_env = shell_path_env();

    let (managed, _foreign) = scan_existing_entries();
    if let Some((link, target)) = managed {
        let resolved = std::fs::canonicalize(&target).ok();
        let stale = resolved.as_deref() != Some(running.as_path());
        return Ok(CliToolStatus {
            installed: true,
            link_path: Some(link.display().to_string()),
            target_path: Some(target.display().to_string()),
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
        match inspect_entry(&link) {
            ExistingEntry::Absent => {}
            ExistingEntry::ManagedLink(_) => {
                // Ours (or a previous install pointing at a moved app) — replace it.
                std::fs::remove_file(&link)
                    .map_err(|e| format!("Cannot replace {}: {e}", link.display()))?;
            }
            ExistingEntry::Foreign => {
                return Err(foreign_entry_error(&link));
            }
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
    let (managed, foreign) = scan_existing_entries();
    match managed {
        Some((link, _target)) => {
            std::fs::remove_file(&link)
                .map_err(|e| format!("Cannot remove {}: {e}", link.display()))?;
        }
        None => {
            if let Some(link) = foreign.first() {
                return Err(format!(
                    "{} is not a Cull app bundle link, so Cull will not remove it. Remove it yourself if it is in the way.",
                    link.display()
                ));
            }
        }
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

    #[test]
    fn writability_probe_leaves_no_files_behind() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(is_user_writable(tmp.path()));
        assert!(std::fs::read_dir(tmp.path()).unwrap().next().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_directory_the_current_user_cannot_write_to() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let tmp = tempfile::tempdir().unwrap();
        // Infer the current euid from a file we just created (no libc needed):
        // root can write into read-only directories, so the expectation below
        // only holds for unprivileged users.
        let whoami = tmp.path().join("whoami");
        std::fs::write(&whoami, b"").unwrap();
        if std::fs::metadata(&whoami).unwrap().uid() == 0 {
            return;
        }
        let dir = tmp.path().join("restricted");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert!(!is_user_writable(&dir));
    }

    #[test]
    fn child_of_a_file_is_not_writable() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("occupied.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(!is_user_writable(&file.join("bin")));
    }

    fn candidate(dir: &str, on_path: bool, writable: bool, blocked: bool) -> Candidate {
        Candidate {
            dir: PathBuf::from(dir),
            on_path,
            writable,
            blocked,
        }
    }

    #[test]
    fn prefers_writable_candidate_already_on_path() {
        let candidates = vec![
            candidate("/opt/homebrew/bin", true, true, false),
            candidate("/usr/local/bin", true, true, false),
            candidate("~/.local/bin", false, true, false),
        ];
        let chosen = choose_candidate(&candidates).unwrap();
        assert_eq!(chosen.dir, PathBuf::from("/opt/homebrew/bin"));
    }

    #[test]
    fn skips_on_path_candidate_that_cannot_be_written() {
        let candidates = vec![
            candidate("/opt/homebrew/bin", false, false, false),
            candidate("/usr/local/bin", true, false, false),
            candidate("~/.local/bin", false, true, false),
        ];
        let chosen = choose_candidate(&candidates).unwrap();
        assert_eq!(chosen.dir, PathBuf::from("~/.local/bin"));
    }

    #[test]
    fn skips_directory_occupied_by_an_unrelated_executable() {
        let candidates = vec![
            candidate("/opt/homebrew/bin", true, true, true),
            candidate("~/.local/bin", false, true, false),
        ];
        let chosen = choose_candidate(&candidates).unwrap();
        assert_eq!(chosen.dir, PathBuf::from("~/.local/bin"));
    }

    #[test]
    fn still_chooses_writable_candidate_when_every_one_is_occupied() {
        let candidates = vec![
            candidate("/opt/homebrew/bin", true, true, true),
            candidate("/usr/local/bin", true, false, true),
        ];
        let chosen = choose_candidate(&candidates).unwrap();
        assert_eq!(chosen.dir, PathBuf::from("/opt/homebrew/bin"));
    }

    #[test]
    fn fails_when_no_candidate_is_writable() {
        let candidates = vec![
            candidate("/opt/homebrew/bin", true, false, false),
            candidate("/usr/local/bin", true, false, false),
            candidate("~/.local/bin", false, false, false),
        ];
        assert!(choose_candidate(&candidates).is_none());
    }

    #[test]
    fn quotes_paths_for_shell_profiles() {
        assert_eq!(
            shell_quote("/Users/test/.local/bin"),
            "'/Users/test/.local/bin'"
        );
        assert_eq!(shell_quote("/Users/my user/bin"), "'/Users/my user/bin'");
        assert_eq!(shell_quote("/Users/it's/bin"), "'/Users/it'\\''s/bin'");
    }

    #[test]
    fn builds_path_hint_only_for_off_path_directories() {
        std::env::set_var("HOME", "/Users/test");
        let dir = PathBuf::from("/Users/test/.local/bin");
        assert_eq!(
            path_hint_for(&dir, "/usr/bin:/bin"),
            Some("export PATH='/Users/test/.local/bin':$PATH".to_string())
        );
        assert_eq!(path_hint_for(&dir, "/usr/bin:~/.local/bin"), None);
        assert_eq!(path_hint_for(&dir, "/usr/bin:/Users/test/.local/bin"), None);
    }

    #[test]
    fn recognizes_cull_bundle_link_targets() {
        assert!(is_cull_bundle_link(Path::new(
            "/Applications/Cull.app/Contents/MacOS/cull"
        )));
        // Old/moved app locations still count.
        assert!(is_cull_bundle_link(Path::new(
            "/Volumes/Old/Apps/Cull.app/Contents/MacOS/cull"
        )));
        assert!(is_cull_bundle_link(Path::new(
            "/Users/test/Applications/Cull.app/Contents/MacOS/cull"
        )));
        // Homebrew cask layout.
        assert!(is_cull_bundle_link(Path::new(
            "/opt/homebrew/Caskroom/cull/0.6.2/Cull.app/Contents/MacOS/cull"
        )));
        // Anything else does not.
        assert!(!is_cull_bundle_link(Path::new("/usr/local/bin/cull")));
        assert!(!is_cull_bundle_link(Path::new("/usr/local/bin/other-tool")));
        assert!(!is_cull_bundle_link(Path::new(
            "/Applications/Cull.app/Contents/Resources/cull"
        )));
    }

    #[cfg(unix)]
    #[test]
    fn unrelated_symlink_is_foreign_and_never_manageable() {
        let tmp = tempfile::tempdir().unwrap();
        let link = tmp.path().join("cull");
        std::os::unix::fs::symlink("/usr/local/bin/some-other-tool", &link).unwrap();
        assert!(matches!(inspect_entry(&link), ExistingEntry::Foreign));
        // And such an entry blocks the directory when alternatives exist.
        assert!(matches!(
            inspect_entry(&tmp.path().join("missing")),
            ExistingEntry::Absent
        ));
        let file = tmp.path().join("real-file");
        std::fs::write(&file, b"x").unwrap();
        assert!(matches!(inspect_entry(&file), ExistingEntry::Foreign));
    }

    #[cfg(unix)]
    #[test]
    fn stale_cull_bundle_link_remains_repairable() {
        let tmp = tempfile::tempdir().unwrap();
        let link = tmp.path().join("cull");
        // A link to an app bundle that has since been moved or deleted.
        std::os::unix::fs::symlink("/Volumes/Old/Apps/Cull.app/Contents/MacOS/cull", &link)
            .unwrap();
        match inspect_entry(&link) {
            ExistingEntry::ManagedLink(target) => assert_eq!(
                target,
                PathBuf::from("/Volumes/Old/Apps/Cull.app/Contents/MacOS/cull")
            ),
            _ => panic!("stale Cull bundle link must stay manageable"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn homebrew_cask_style_link_is_manageable() {
        let tmp = tempfile::tempdir().unwrap();
        let link = tmp.path().join("cull");
        std::os::unix::fs::symlink(
            "/opt/homebrew/Caskroom/cull/0.6.2/Cull.app/Contents/MacOS/cull",
            &link,
        )
        .unwrap();
        assert!(matches!(
            inspect_entry(&link),
            ExistingEntry::ManagedLink(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn scan_reports_managed_link_and_leaves_foreign_entries_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let first = tmp.path().join("first");
        let second = tmp.path().join("second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        // An unrelated symlink in the first candidate directory…
        let foreign_link = first.join("cull");
        std::os::unix::fs::symlink("/usr/local/bin/some-other-tool", &foreign_link).unwrap();
        // …and a stale Cull link in the second one.
        let stale_link = second.join("cull");
        std::os::unix::fs::symlink(
            "/Volumes/Old/Apps/Cull.app/Contents/MacOS/cull",
            &stale_link,
        )
        .unwrap();

        let (managed, foreign) = scan_existing_entries_in(&[first.clone(), second.clone()]);
        assert_eq!(
            managed,
            Some((
                stale_link.clone(),
                PathBuf::from("/Volumes/Old/Apps/Cull.app/Contents/MacOS/cull")
            ))
        );
        assert_eq!(foreign, vec![foreign_link.clone()]);

        // The foreign symlink itself is untouched by recognition.
        assert_eq!(
            std::fs::read_link(&foreign_link).unwrap(),
            PathBuf::from("/usr/local/bin/some-other-tool")
        );
    }

    #[cfg(unix)]
    #[test]
    fn scan_without_entries_finds_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let (managed, foreign) = scan_existing_entries_in(&[tmp.path().to_path_buf()]);
        assert_eq!(managed, None);
        assert!(foreign.is_empty());
    }
}
