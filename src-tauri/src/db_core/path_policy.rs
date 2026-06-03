// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! Unified filesystem path policy.
//!
//! Paths enter Cull from several surfaces — deep links, user-picked import
//! folders, clipboard paste destinations — and previously each applied its own
//! ad-hoc rules. This module is the single source of truth: every surface calls
//! [`validate_path`] with the [`PathMode`] that describes its trust level, so the
//! rules are explicit and consistent.

use std::path::{Component, Path, PathBuf};

/// How a path entered the app, which determines how strictly it is validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathMode {
    /// Paths from `cull://` deep links — least trusted. Must be under $HOME, not
    /// a sensitive directory, and contain no hidden (dot) components.
    Deeplink,
    /// Paths the user explicitly chose in a native picker (import folder,
    /// clipboard paste destination). Trusted to be intentional, but still
    /// blocked from sensitive directories to avoid accidental exposure.
    UserPicked,
}

/// Sensitive directories (relative to $HOME) that must never be traversed,
/// regardless of how the path entered the app.
const SENSITIVE_DIRS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".config/gcloud",
    "Library/Keychains",
];

/// Validate a path under the given mode, resolving symlinks and `..` first.
/// Returns the canonicalized path on success or a user-facing reason on refusal.
pub fn validate_path(raw: &str, mode: PathMode) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    validate_path_with_home(raw, mode, &home)
}

fn validate_path_with_home(raw: &str, mode: PathMode, home: &Path) -> Result<PathBuf, String> {
    // Canonicalize resolves symlinks and normalizes "..", so traversal escapes
    // are caught by the under-home check below.
    let canonical =
        std::fs::canonicalize(raw).map_err(|e| format!("Cannot resolve path '{}': {}", raw, e))?;

    let under_home = canonical.starts_with(home);

    if mode == PathMode::Deeplink && !under_home {
        return Err(format!(
            "Path '{}' is outside the home directory",
            canonical.display()
        ));
    }

    // Sensitive-directory and hidden-component checks apply to the portion under
    // $HOME. Paths outside $HOME are only reachable via UserPicked (explicit
    // choice) and are not dotfile-screened.
    if let Ok(relative) = canonical.strip_prefix(home) {
        // Case-insensitive prefix match: macOS (APFS) is case-insensitive by
        // default, so `~/.SSH` resolves to `~/.ssh`. Compare on lowercased,
        // '/'-joined components with a path boundary so ".sshfoo" is not matched.
        let rel_lower = relative.to_string_lossy().to_lowercase();
        for sensitive in SENSITIVE_DIRS {
            let s = sensitive.to_lowercase();
            if rel_lower == s || rel_lower.starts_with(&format!("{}/", s)) {
                return Err(format!(
                    "Access to '{}' is blocked (sensitive directory)",
                    canonical.display()
                ));
            }
        }

        if mode == PathMode::Deeplink {
            for component in relative.components() {
                if let Component::Normal(name) = component {
                    if name.to_str().map_or(false, |s| s.starts_with('.')) {
                        return Err(format!(
                            "Access to '{}' is blocked (hidden path component '{}')",
                            canonical.display(),
                            name.to_string_lossy()
                        ));
                    }
                }
            }
        }
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mkdir(base: &Path, rel: &str) -> PathBuf {
        let p = base.join(rel);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// Canonicalized tempdir, so prefix comparisons match the canonicalized
    /// paths the policy produces (on macOS /var/... resolves to /private/var/...).
    /// Production uses `dirs::home_dir()`, which is already canonical.
    fn canonical_tempdir() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let canon = std::fs::canonicalize(tmp.path()).unwrap();
        (tmp, canon)
    }

    #[test]
    fn deeplink_accepts_normal_dir_under_home() {
        let (_home, home) = canonical_tempdir();
        let dir = mkdir(&home, "Pictures/art");
        let ok = validate_path_with_home(dir.to_str().unwrap(), PathMode::Deeplink, &home);
        assert!(ok.is_ok(), "{:?}", ok);
    }

    #[test]
    fn deeplink_rejects_outside_home() {
        let (_home, home) = canonical_tempdir();
        let (_other, other) = canonical_tempdir();
        let dir = mkdir(&other, "stuff");
        let err =
            validate_path_with_home(dir.to_str().unwrap(), PathMode::Deeplink, &home).unwrap_err();
        assert!(err.contains("outside the home directory"), "{err}");
    }

    #[test]
    fn deeplink_rejects_sensitive_dir() {
        let (_home, home) = canonical_tempdir();
        let dir = mkdir(&home, ".ssh");
        let err =
            validate_path_with_home(dir.to_str().unwrap(), PathMode::Deeplink, &home).unwrap_err();
        assert!(err.contains("sensitive directory"), "{err}");
    }

    #[test]
    fn deeplink_rejects_hidden_component() {
        let (_home, home) = canonical_tempdir();
        let dir = mkdir(&home, ".secrets/data");
        let err =
            validate_path_with_home(dir.to_str().unwrap(), PathMode::Deeplink, &home).unwrap_err();
        assert!(err.contains("hidden path component"), "{err}");
    }

    #[test]
    fn userpicked_allows_outside_home_and_hidden() {
        let (_home, home) = canonical_tempdir();
        let (_other, other) = canonical_tempdir();
        let outside = mkdir(&other, "external");
        assert!(
            validate_path_with_home(outside.to_str().unwrap(), PathMode::UserPicked, &home).is_ok()
        );
        // Hidden components are fine for an explicitly chosen folder...
        let hidden = mkdir(&home, ".myapp/cache");
        assert!(
            validate_path_with_home(hidden.to_str().unwrap(), PathMode::UserPicked, &home).is_ok()
        );
    }

    #[test]
    fn userpicked_still_rejects_sensitive_dir() {
        // ...but never a sensitive directory, even when explicitly chosen.
        let (_home, home) = canonical_tempdir();
        let dir = mkdir(&home, ".gnupg");
        let err = validate_path_with_home(dir.to_str().unwrap(), PathMode::UserPicked, &home)
            .unwrap_err();
        assert!(err.contains("sensitive directory"), "{err}");
    }

    #[test]
    fn sensitive_dir_match_is_case_insensitive() {
        // macOS APFS is case-insensitive: ~/.SSH resolves to ~/.ssh and must
        // still be blocked despite the differing case.
        let (_home, home) = canonical_tempdir();
        let dir = mkdir(&home, ".SSH");
        let err =
            validate_path_with_home(dir.to_str().unwrap(), PathMode::Deeplink, &home).unwrap_err();
        assert!(
            err.contains("sensitive directory") || err.contains("hidden path component"),
            "{err}"
        );
        // And under UserPicked (no hidden screening) it must hit the sensitive rule.
        let dir2 = mkdir(&home, "Library/Keychains/sub");
        // also exercise a mixed-case variant of a non-dot sensitive dir
        let mixed = mkdir(&home, "library/keychains/x");
        assert!(
            validate_path_with_home(dir2.to_str().unwrap(), PathMode::UserPicked, &home).is_err()
        );
        assert!(
            validate_path_with_home(mixed.to_str().unwrap(), PathMode::UserPicked, &home).is_err(),
            "case-insensitive Library/Keychains must be blocked"
        );
    }

    #[test]
    fn nonexistent_path_is_rejected() {
        let (_home, home) = canonical_tempdir();
        let missing = home.join("does/not/exist");
        assert!(
            validate_path_with_home(missing.to_str().unwrap(), PathMode::Deeplink, &home).is_err()
        );
    }
}
