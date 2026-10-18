//! API-key resolution shared by the data sources.
//!
//! A key can come from several places and each source has its own list, but the
//! mechanism is the same: read a process environment variable, and if it is
//! unset fall back to asking the user's login + interactive shell. The shell
//! fallback exists because a packaged `.app` (Finder / `open`) inherits no shell
//! environment, and a running process cannot see a variable set after it
//! started — asking `$SHELL -lic` is the only way "set the var, then hit
//! refresh" works without a restart.
//!
//! One implementation, parameterised by the variable name, replaces the
//! near-copies each source used to carry.

use std::process::{Command, Stdio};

/// One key source: the environment variable, the marker the shell probe prints
/// before its value, and the hint shown when nothing is configured.
pub struct Spec<'a> {
    /// The environment variable to read.
    pub env_var: &'a str,
    /// A unique token the shell probe prints directly before the value, so
    /// interactive startup noise on stdout can be discarded.
    pub marker: &'a str,
    /// The message shown when no source yields a key.
    pub missing: &'a str,
}

/// Trims surrounding whitespace and drops empty results, so a blank or
/// newline-padded value counts as unconfigured rather than a bad key.
pub fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

/// The key to fetch with: `first` (a source-specific store such as OpenCode's
/// `auth.json`, or `None`), then the process environment, then the login shell.
///
/// Without a key anywhere this is `spec.missing`, the error the UI shows
/// verbatim.
pub fn resolve(spec: &Spec<'_>, first: Option<String>) -> Result<String, String> {
    first
        .or_else(|| non_blank(std::env::var(spec.env_var).ok()))
        .or_else(|| from_shell(spec.env_var, spec.marker))
        .ok_or_else(|| spec.missing.to_string())
}

/// Re-reads `env_var` from the user's login + interactive shell, `None` when it
/// yields nothing.
///
/// The `-i` matters: exports usually live in `~/.zshrc` / `~/.bashrc`, which a
/// plain `zsh -lc` never sources. Interactive setups may print their own noise
/// to stdout, so the value is fetched behind `marker` and everything before it
/// is ignored. Unix only; elsewhere the process env is all there is.
pub fn from_shell(env_var: &str, marker: &str) -> Option<String> {
    let shell = std::env::var("SHELL").ok()?;
    let output = Command::new(shell)
        .args(["-lic", &format!("printf %s \"{marker}${{{env_var}}}\"")])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `rsplit` lands on the chunk after the *last* marker, i.e. our value.
    non_blank(stdout.rsplit(marker).next().map(String::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A spec whose env var is never set in the test environment.
    const SPEC: Spec<'static> = Spec {
        env_var: "IGUI_TEST_KEY_UNSET",
        marker: "__IGUI_TEST__",
        missing: "未配置密钥",
    };

    #[test]
    fn blank_values_count_as_unconfigured() {
        assert_eq!(non_blank(None), None);
        assert_eq!(non_blank(Some(String::new())), None);
        assert_eq!(non_blank(Some("   ".to_string())), None);
        assert_eq!(
            non_blank(Some("sk-test".to_string())),
            Some("sk-test".into())
        );
        assert_eq!(
            non_blank(Some("  sk-test\n".to_string())),
            Some("sk-test".into())
        );
    }

    /// A source-specific store wins without touching the environment or shell.
    #[test]
    fn a_preferred_key_wins() {
        assert_eq!(
            resolve(&SPEC, Some("from-file".to_string())),
            Ok("from-file".into())
        );
    }

    /// With no store, the process environment is the next source.
    #[test]
    fn the_process_env_is_the_next_source() {
        std::env::set_var(SPEC.env_var, "  from-env  ");
        assert_eq!(resolve(&SPEC, None), Ok("from-env".into()));
        std::env::remove_var(SPEC.env_var);
    }
}
