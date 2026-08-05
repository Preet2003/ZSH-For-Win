//! Theme package resolve, validation, and built-in themes.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use winzsh_core::ThemeId;
use winzsh_error::{Result, message};

/// Resolved active theme.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Theme {
    /// Theme identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Color palette (PowerShell `$PSStyle` expressions).
    pub palette: Palette,
    /// Glyphs used by the prompt.
    pub symbols: Symbols,
}

/// Prompt color slots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Palette {
    /// Path / cwd color expression.
    pub path: String,
    /// Clean git segment color.
    pub git_clean: String,
    /// Dirty git segment color.
    pub git_dirty: String,
    /// Prompt character color.
    pub prompt: String,
    /// Reset expression.
    pub reset: String,
}

/// Prompt symbols.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Symbols {
    /// Primary prompt character.
    pub prompt: String,
    /// Dirty working tree marker.
    pub git_dirty: String,
    /// Clean working tree marker (may be empty).
    pub git_clean: String,
}

/// Resolved active theme wrapper.
#[derive(Debug, Clone)]
pub struct ResolvedTheme {
    /// Theme identifier.
    pub id: ThemeId,
    /// Full theme document.
    pub theme: Theme,
}

/// Built-in theme ids in display order.
pub const BUILTIN_IDS: &[&str] = &[
    "minimal",
    "classic",
    "powerline",
    "modern",
    "catppuccin",
    "tokyo-night",
];

/// List built-in theme ids.
pub fn builtin_ids() -> Vec<&'static str> {
    BUILTIN_IDS.to_vec()
}

/// All built-in themes.
pub fn builtin_themes() -> Vec<Theme> {
    vec![
        named(
            "minimal", "Minimal", "White", "Green", "Yellow", "White", ">", "*", "",
        ),
        named(
            "classic", "Classic", "Cyan", "Green", "Red", "Blue", "$", "✗", "✔",
        ),
        named(
            "powerline",
            "Powerline",
            "BrightBlue",
            "BrightGreen",
            "BrightYellow",
            "BrightMagenta",
            ">",
            "*",
            "",
        ),
        named(
            "modern", "Modern", "Cyan", "Green", "Yellow", "Magenta", "❯", "*", "",
        ),
        rgb_theme(
            "catppuccin",
            "Catppuccin Mocha",
            (137, 180, 250),
            (166, 227, 161),
            (249, 226, 175),
            (203, 166, 247),
            "❯",
            "•",
            "",
        ),
        rgb_theme(
            "tokyo-night",
            "Tokyo Night",
            (122, 162, 247),
            (158, 206, 106),
            (224, 175, 104),
            (187, 154, 247),
            "❯",
            "✗",
            "✔",
        ),
    ]
}

/// Resolve a theme id from built-ins (installed custom themes land later).
pub fn resolve(id: &str) -> Result<ResolvedTheme> {
    let id = id.trim();
    if id.is_empty() {
        return Err(message("theme id must not be empty"));
    }
    builtin_themes()
        .into_iter()
        .find(|t| t.id.eq_ignore_ascii_case(id))
        .map(|theme| ResolvedTheme {
            id: ThemeId(theme.id.clone()),
            theme,
        })
        .ok_or_else(|| {
            message(format!(
                "unknown theme '{id}'; known: {}",
                BUILTIN_IDS.join(", ")
            ))
        })
}

/// Validate that a theme id exists.
pub fn validate_id(id: &str) -> Result<()> {
    resolve(id).map(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn named(
    id: &str,
    name: &str,
    path: &str,
    git_clean: &str,
    git_dirty: &str,
    prompt: &str,
    prompt_sym: &str,
    dirty_sym: &str,
    clean_sym: &str,
) -> Theme {
    Theme {
        id: id.to_string(),
        name: name.to_string(),
        palette: Palette {
            path: format!("$($PSStyle.Foreground.{path})"),
            git_clean: format!("$($PSStyle.Foreground.{git_clean})"),
            git_dirty: format!("$($PSStyle.Foreground.{git_dirty})"),
            prompt: format!("$($PSStyle.Foreground.{prompt})"),
            reset: "$($PSStyle.Reset)".to_string(),
        },
        symbols: Symbols {
            prompt: prompt_sym.to_string(),
            git_dirty: dirty_sym.to_string(),
            git_clean: clean_sym.to_string(),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn rgb_theme(
    id: &str,
    name: &str,
    path: (u8, u8, u8),
    git_clean: (u8, u8, u8),
    git_dirty: (u8, u8, u8),
    prompt: (u8, u8, u8),
    prompt_sym: &str,
    dirty_sym: &str,
    clean_sym: &str,
) -> Theme {
    Theme {
        id: id.to_string(),
        name: name.to_string(),
        palette: Palette {
            path: rgb(path),
            git_clean: rgb(git_clean),
            git_dirty: rgb(git_dirty),
            prompt: rgb(prompt),
            reset: "$($PSStyle.Reset)".to_string(),
        },
        symbols: Symbols {
            prompt: prompt_sym.to_string(),
            git_dirty: dirty_sym.to_string(),
            git_clean: clean_sym.to_string(),
        },
    }
}

fn rgb(c: (u8, u8, u8)) -> String {
    format!("$($PSStyle.Foreground.FromRgb({},{},{}))", c.0, c.1, c.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_modern() {
        let t = resolve("modern").expect("theme");
        assert_eq!(t.theme.id, "modern");
        assert!(!t.theme.symbols.prompt.is_empty());
    }

    #[test]
    fn rejects_unknown() {
        assert!(resolve("nope").is_err());
    }
}
