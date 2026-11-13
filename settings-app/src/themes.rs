use lightningcss::error::ParserError;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

const MAX_THEME_BYTES: u64 = 1024 * 1024;
const MAX_CSS_DIAGNOSTICS_PER_THEME: usize = 8;
const DEFAULT_BASE_COLOR: &str = "#191918";

const DARK_MODE_PLUS: &str = include_str!("../themes/built-in-themes/dark-mode-plus.css");
const OLED: &str = include_str!("../themes/built-in-themes/oled.css");
const LEGACY_DARK_MODE_PLUS: &str = r#"/*
author: PainterFlow11
name: Dark Mode+
based-theme: Dark Mode
accent-color: #DE50DE
accent-color-selections: true
*/
/* Dark Mode+ keeps the background near black while preserving Anytype's surface contrast. */
/* The object discussion footer fade ends at the main background color. */
html, html.themeDark {
  --color-bg-primary: #191918;
  --color-bg-primary-90: rgba(25, 25, 24, 0.9);
  --color-bg-secondary: #1c1c1b;
  --blobui-discussion-footer-fade-background: linear-gradient(180deg, transparent 3.62%, var(--color-bg-primary) 68.18%);
}
"#;
const LEGACY_OLED: &str = r#"/*
author: PainterFlow11
name: OLED
based-theme: Dark Mode
accent-color: #DE50DE
accent-color-selections: true
*/
/* OLED uses a pure black background and disables the object discussion footer fade. */
html, html.themeDark {
  --color-bg-primary: #000000;
  --color-bg-primary-90: rgba(0, 0, 0, 0.9);
  --color-bg-secondary: #000000;
  --color-shape-primary: #171717;
  --color-shape-secondary: #101010;
  --color-shape-tertiary: #080808;
  --blobui-discussion-footer-fade-background: none;
}
"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThemeSummary {
    pub id: String,
    pub name: String,
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent_color_selection: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThemeDiagnostic {
    pub message: String,
    pub level: ThemeDiagnosticLevel,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ThemeDiagnosticLevel {
    Warning,
    Error,
}

impl ThemeDiagnostic {
    pub(crate) fn error(message: impl Into<String>) -> Self {
        Self { message: message.into(), level: ThemeDiagnosticLevel::Error }
    }
}

#[derive(Debug, Clone)]
struct ThemeFile {
    summary: ThemeSummary,
    css: String,
}

#[derive(Debug)]
pub(crate) struct ThemeCatalog {
    pub(crate) themes: Vec<ThemeSummary>,
    pub(crate) diagnostics: Vec<ThemeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThemeSourceError {
    line: usize,
    column: usize,
    message: String,
}

#[derive(Debug)]
struct ThemeFileLoad {
    theme: Option<ThemeFile>,
    diagnostics: Vec<ThemeDiagnostic>,
}

#[derive(Debug)]
enum ThemeFileInspectionError {
    NotFound(String),
    Other(String),
}

impl ThemeFileInspectionError {
    fn message(&self) -> &str {
        match self {
            Self::NotFound(message) | Self::Other(message) => message,
        }
    }
}

impl From<String> for ThemeFileInspectionError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

#[derive(Debug)]
struct ThemeMetadata {
    name: String,
    author: String,
    accent_color: Option<String>,
    accent_color_selection: Option<bool>,
}

pub(crate) fn theme_directory(settings_directory: &Path) -> PathBuf {
    settings_directory.join("themes").join("built-in-themes")
}

pub(crate) fn clear_missing_selected_theme(settings_directory: &Path, theme_id: &mut String) {
    if theme_id.is_empty() {
        return;
    }
    let Ok(path) = selected_theme_path(settings_directory, theme_id) else {
        return;
    };
    if matches!(fs::symlink_metadata(path), Err(error) if error.kind() == ErrorKind::NotFound) {
        theme_id.clear();
    }
}

pub(crate) fn ensure_builtin_themes(settings_directory: &Path) -> Result<(), String> {
    ensure_directory(settings_directory)?;
    let themes = settings_directory.join("themes");
    ensure_directory(&themes)?;
    let built_in = themes.join("built-in-themes");
    ensure_directory(&built_in)?;
    write_builtin_if_missing(&built_in.join("dark-mode-plus.css"), DARK_MODE_PLUS, LEGACY_DARK_MODE_PLUS)?;
    write_builtin_if_missing(&built_in.join("oled.css"), OLED, LEGACY_OLED)?;
    Ok(())
}

pub(crate) fn list_themes(settings_directory: &Path) -> Result<ThemeCatalog, String> {
    ensure_builtin_themes(settings_directory)?;
    let directory = theme_directory(settings_directory);
    let entries = fs::read_dir(&directory).map_err(|error| {
        format!(
            "Could not read custom themes from {}: {error}",
            directory.display()
        )
    })?;
    let mut themes = Vec::new();
    let mut diagnostics = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Could not inspect custom theme files: {error}"))?;
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("css"))
            || !path.is_file()
        {
            continue;
        }
        match inspect_theme_file_for_catalog(&path) {
            Ok(Some(loaded)) => {
                diagnostics.extend(loaded.diagnostics);
                if let Some(theme) = loaded.theme {
                    themes.push(theme.summary);
                }
            }
            Ok(None) => {}
            Err(error) => diagnostics.push(ThemeDiagnostic::error(theme_file_error(&path, &error))),
        }
    }
    themes.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then(left.id.cmp(&right.id))
    });
    Ok(ThemeCatalog { themes, diagnostics })
}

pub(crate) fn render_generated_css(
    settings_directory: &Path,
    data: &Value,
) -> Result<String, String> {
    let include_selections = effective_include_selections(settings_directory, data)?;
    render_generated_css_with_include(settings_directory, data, include_selections)
}

pub(crate) fn render_generated_css_with_include(
    settings_directory: &Path,
    data: &Value,
    include_selections: bool,
) -> Result<String, String> {
    ensure_builtin_themes(settings_directory)?;
    let empty_settings = Map::new();
    let settings = data
        .get("settings")
        .and_then(Value::as_object)
        .unwrap_or(&empty_settings);
    let enable_custom_themes = settings
        .get("enableCustomThemes")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let accent = crate::css::normalize_accent(
        settings
            .get("accentColor")
            .and_then(Value::as_str)
            .unwrap_or("#DE50DE"),
    )?;
    if !enable_custom_themes {
        return Ok(render_accent_settings(&accent, include_selections));
    }

    let selected_id = settings
        .get("themePreset")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let footer_fade_fallback = if selected_id.eq_ignore_ascii_case("oled.css") {
        "none"
    } else {
        "linear-gradient(180deg, transparent 3.62%, var(--color-bg-primary) 68.18%)"
    };
    let base_theme = settings
        .get("basedTheme")
        .and_then(Value::as_str)
        .unwrap_or("auto");
    let base_color = normalize_theme_color(
        settings
            .get("themeColor")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_BASE_COLOR),
    )?;
    let preset = if selected_id.is_empty() {
        None
    } else {
        let path = selected_theme_path(settings_directory, selected_id)?;
        read_theme_file_if_present(&path)?
    };
    let resolved_theme = resolved_palette_mode(base_theme, &base_color)?;

    let mut css = String::from(
        "/* Generated by BlobUI Custom Themes. Edit the source theme file instead. */\n",
    );
    if let Some(theme) = resolved_theme {
        css.push_str(&render_palette(&base_color, theme)?);
        css.push('\n');
        css.push_str(&format!("html {{ color-scheme: {theme}; }}\n"));
    } else {
        css.push_str(&render_palette_for_selector(
            &base_color,
            "light",
            "html:not(.themeDark)",
        )?);
        css.push('\n');
        css.push_str(&render_palette_for_selector(
            &base_color,
            "dark",
            "html.themeDark",
        )?);
    }

    if let Some(theme) = preset.as_ref() {
        if !theme.css.trim().is_empty() {
            css.push_str("\n/* Preset and user theme overrides. */\n");
            css.push_str(theme.css.trim());
            css.push('\n');
        }
    }

    // Keep shared icons dark on light palettes, even when a preset defines its own icon color.
    if resolved_theme == Some("light") {
        css.push_str("\nhtml, html.themeDark { --color-icon: #252525; --color-icon-hover: #000000; }\n");
    } else if resolved_theme.is_none() {
        css.push_str("\nhtml:not(.themeDark) { --color-icon: #252525; --color-icon-hover: #000000; }\n");
    }

    // The upstream sidebar fades start with transparent white. That tint can show through
    // colored themes, so use transparent black and the active theme's actual surface color.
    css.push_str("\n/* Theme-aware sidebar fades. */\n");
    css.push_str(".sidebarPage > .bottom .grad { background: linear-gradient(180deg, transparent 3.62%, var(--color-bg-primary) 68.18%); }\n");
    css.push_str(".sidebarPage.pageVault > .bottom .grad { background-image: linear-gradient(180deg, transparent 3.62%, var(--color-shape-highlight-light-solid) 100%); }\n");

    // Theme files can override the fallback to disable or customize the object discussion footer fade.
    css.push_str(&format!("html.themeDark #page .socialBlock {{ background: var(--blobui-discussion-footer-fade-background, {footer_fade_fallback}); }}\n"));

    let accent = preset
        .as_ref()
        .and_then(|theme| theme.summary.accent_color.as_deref())
        .map(crate::css::normalize_accent)
        .transpose()?
        .unwrap_or(accent);
    css.push_str(&render_accent_settings(&accent, include_selections));
    Ok(css)
}

fn render_accent_settings(accent: &str, include_selections: bool) -> String {
    let mut css = String::from("\n:root, html, html.themeDark {\n");
    css.push_str(&format!("  --color-blob-accent: {accent};\n"));
    if include_selections {
        css.push_str("  --color-control-accent: var(--color-blob-accent);\n");
    }
    css.push_str("}\n");
    css
}

pub(crate) fn effective_include_selections(settings_directory: &Path, data: &Value) -> Result<bool, String> {
    let settings = data.get("settings").and_then(Value::as_object);
    let user_setting = settings
        .and_then(|settings| settings.get("includeSelections"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let Some(settings) = settings else { return Ok(user_setting) };
    if !settings
        .get("enableCustomThemes")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(user_setting);
    }
    let Some(theme_id) = settings.get("themePreset").and_then(Value::as_str).filter(|id| !id.is_empty()) else {
        return Ok(user_setting);
    };
    ensure_builtin_themes(settings_directory)?;
    let theme = read_theme_file_if_present(&selected_theme_path(settings_directory, theme_id)?)?;
    Ok(theme
        .and_then(|theme| theme.summary.accent_color_selection)
        .unwrap_or(user_setting))
}

pub(crate) fn preview_mode(data: &Value) -> Result<String, String> {
    let settings = data.get("settings").and_then(Value::as_object);
    if !settings
        .and_then(|settings| settings.get("enableCustomThemes"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok("system".into());
    }
    let base_theme = settings
        .and_then(|settings| settings.get("basedTheme"))
        .and_then(Value::as_str)
        .unwrap_or("auto");
    let base_color = normalize_theme_color(
        settings
            .and_then(|settings| settings.get("themeColor"))
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_BASE_COLOR),
    )?;

    match resolved_palette_mode(base_theme, &base_color)? {
        Some(mode) => Ok(mode.into()),
        None => Ok("system".into()),
    }
}

fn resolved_palette_mode(base_theme: &str, base_color: &str) -> Result<Option<&'static str>, String> {
    match base_theme {
        "light" => Ok(Some("light")),
        "dark" => Ok(Some("dark")),
        "inherit" => Ok(None),
        "auto" => Ok(Some(if is_dark_color(base_color) { "dark" } else { "light" })),
        _ => Err("Based Theme must be Auto, Light Mode, Dark Mode, or Don't Override.".into()),
    }
}

pub(crate) fn validate_theme_color(value: &str) -> Result<String, String> {
    normalize_theme_color(value)
}

fn normalize_theme_color(value: &str) -> Result<String, String> {
    let value = value.trim();
    let Some(digits) = value.strip_prefix('#') else {
        return Err("Theme color must be a three or six digit hexadecimal color.".into());
    };
    if digits.len() == 3 && digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(format!(
            "#{0}{0}{1}{1}{2}{2}",
            &digits[0..1],
            &digits[1..2],
            &digits[2..3]
        )
        .to_ascii_uppercase());
    }
    if digits.len() == 6 && digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(value.to_ascii_uppercase());
    }
    Err("Theme color must be a three or six digit hexadecimal color.".into())
}

fn is_dark_color(color: &str) -> bool {
    let (red, green, blue) = parse_hex(color);
    let linear = |channel: u8| {
        let channel = f64::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue) < 0.5
}

fn render_palette(color: &str, mode: &str) -> Result<String, String> {
    let selector = "html, html.themeDark";
    render_palette_for_selector(color, mode, selector)
}

fn render_palette_for_selector(color: &str, mode: &str, selector: &str) -> Result<String, String> {
    let (red, green, blue) = parse_hex(color);
    let is_dark = mode == "dark";
    let (
        background_secondary,
        shape_primary,
        shape_secondary,
        shape_tertiary,
        highlight,
        text,
        secondary_text,
        tertiary_text,
        inversion_text,
        control_accent,
        control_active,
        control_inactive,
        very_light_grey,
        light_grey,
        grey,
        dark_grey,
        bg_inversion,
        shape_inversion,
    ) = if mode == "dark" {
        (
            mix_hex(red, green, blue, (255, 255, 255), 0.01),
            mix_hex(red, green, blue, (255, 255, 255), 0.10),
            mix_hex(red, green, blue, (255, 255, 255), 0.07),
            mix_hex(red, green, blue, (255, 255, 255), 0.035),
            "255, 255, 255",
            "#E1E1E1",
            "#A3A3A3",
            "#5C5C5C",
            "#171717",
            "#D4D4D4",
            "#737373",
            "#414141",
            mix_hex(red, green, blue, (255, 255, 255), 0.06),
            mix_hex(red, green, blue, (255, 255, 255), 0.15),
            "#A8A8A8".to_string(),
            "#C8C8C8".to_string(),
            "#FFFFFF",
            "#292929",
        )
    } else {
        (
            mix_hex(red, green, blue, (0, 0, 0), 0.01),
            mix_hex(red, green, blue, (0, 0, 0), 0.13),
            mix_hex(red, green, blue, (0, 0, 0), 0.08),
            mix_hex(red, green, blue, (0, 0, 0), 0.03),
            "0, 0, 0",
            "#252525",
            "#828282",
            "#BFBFBF",
            "#FFFFFF",
            "#252525",
            "#B6B6B6",
            "#DCDCDC",
            mix_hex(red, green, blue, (0, 0, 0), 0.03),
            mix_hex(red, green, blue, (0, 0, 0), 0.08),
            "#888888".to_string(),
            "#595959".to_string(),
            "#171717",
            "#EBEBEB",
        )
    };
    let rgba = format!("{}, {}, {}", red, green, blue);
    let text_rgb = if is_dark { "225, 225, 225" } else { "37, 37, 37" };
    let bg_loader = if is_dark { "rgba(0, 0, 0, 0.7)" } else { "rgba(255, 255, 255, 0.7)" };
    let attachment_loader = if is_dark { "rgba(255, 255, 255, 0.7)" } else { "rgba(134, 134, 134, 0.7)" };
    let text_primary_20 = if is_dark { "rgba(225, 225, 225, 0.2)" } else { "rgba(0, 0, 0, 0.2)" };
    let icon_hover = if is_dark { "#E1E1E1" } else { "#000000" };
    let icon = if is_dark { "#A09F92" } else { "#252525" };
    Ok(format!(
        "{selector} {{\n  --color-text-primary: {text};\n  --color-text-primary-20: {text_primary_20};\n  --color-text-primary-70: rgba({text_rgb}, 0.75);\n  --color-text-secondary: {secondary_text};\n  --color-text-secondary-transparent: rgba({text_rgb}, 0.6);\n  --color-text-tertiary: {tertiary_text};\n  --color-text-inversion: {inversion_text};\n  --color-text-white: #FFFFFF;\n  --color-text-white60: rgba(255, 255, 255, 0.6);\n  --color-text-primary-inversion: #EBEBEB;\n  --color-text-secondary-inversion: #9A9A9A;\n  --color-text-tertiary-inversion: #5C5C5C;\n  --color-shape-primary: {shape_primary};\n  --color-shape-secondary: {shape_secondary};\n  --color-shape-secondary-inversion: {shape_inversion};\n  --color-shape-tertiary: {shape_tertiary};\n  --color-shape-highlight-dark: rgba({highlight}, 0.11);\n  --color-shape-highlight-medium: rgba({highlight}, 0.05);\n  --color-shape-highlight-light: rgba({highlight}, 0.03);\n  --color-shape-highlight-light-solid: {shape_tertiary};\n  --color-control-accent: {control_accent};\n  --color-control-active: {control_active};\n  --color-control-active-inversion: #737373;\n  --color-control-inactive: {control_inactive};\n  --color-control-bg: #FFFFFF;\n  --color-icon: {icon};\n  --color-icon-hover: {icon_hover};\n  --color-bg-primary: {color};\n  --color-bg-primary-90: rgba({rgba}, 0.9);\n  --color-bg-primary-inversion: {bg_inversion};\n  --color-bg-secondary: {background_secondary};\n  --color-bg-loader: {bg_loader};\n  --color-attachment-loader: {attachment_loader};\n  --color-very-light-grey: {very_light_grey};\n  --color-light-grey: {light_grey};\n  --color-grey: {grey};\n  --color-dark-grey: {dark_grey};\n}}\n"
    ))
}

fn parse_hex(value: &str) -> (u8, u8, u8) {
    let digits = &value[1..];
    (
        u8::from_str_radix(&digits[0..2], 16).expect("validated theme color"),
        u8::from_str_radix(&digits[2..4], 16).expect("validated theme color"),
        u8::from_str_radix(&digits[4..6], 16).expect("validated theme color"),
    )
}

fn mix_hex(red: u8, green: u8, blue: u8, target: (u8, u8, u8), amount: f64) -> String {
    let mix = |from: u8, to: u8| {
        (f64::from(from) * (1.0 - amount) + f64::from(to) * amount).round() as u8
    };
    format!(
        "#{:02X}{:02X}{:02X}",
        mix(red, target.0),
        mix(green, target.1),
        mix(blue, target.2)
    )
}

fn selected_theme_path(settings_directory: &Path, id: &str) -> Result<PathBuf, String> {
    if id.len() > 128
        || id.starts_with('.')
        || !id.to_ascii_lowercase().ends_with(".css")
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b' '))
    {
        return Err("The selected custom theme name is invalid.".into());
    }
    Ok(theme_directory(settings_directory).join(id))
}

fn read_theme_file_if_present(path: &Path) -> Result<Option<ThemeFile>, String> {
    let loaded = match inspect_theme_file(path) {
        Ok(loaded) => loaded,
        Err(ThemeFileInspectionError::NotFound(_)) => return Ok(None),
        Err(error) => return Err(theme_file_error(path, error.message())),
    };
    theme_from_load(path, loaded).map(Some)
}

fn theme_from_load(path: &Path, loaded: ThemeFileLoad) -> Result<ThemeFile, String> {
    loaded.theme.ok_or_else(|| {
        loaded
            .diagnostics
            .into_iter()
            .next()
            .map(|diagnostic| theme_file_error(path, &diagnostic.message))
            .unwrap_or_else(|| format!("{} has an invalid custom theme header.", theme_file_name(path)))
    })
}

fn inspect_theme_file_for_catalog(path: &Path) -> Result<Option<ThemeFileLoad>, String> {
    match inspect_theme_file(path) {
        Ok(loaded) => Ok(Some(loaded)),
        Err(ThemeFileInspectionError::NotFound(_)) => Ok(None),
        Err(error) => Err(error.message().to_string()),
    }
}

fn inspect_theme_file(path: &Path) -> Result<ThemeFileLoad, ThemeFileInspectionError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        let message = format!("Could not inspect theme file '{}': {error}", theme_file_name(path));
        if error.kind() == ErrorKind::NotFound {
            ThemeFileInspectionError::NotFound(message)
        } else {
            ThemeFileInspectionError::Other(message)
        }
    })?;
    reject_reparse_point(&metadata, path)?;
    if !metadata.is_file() || metadata.len() > MAX_THEME_BYTES {
        return Err(format!(
            "Theme file '{}' must be CSS and no larger than 1 MB.",
            theme_file_name(path)
        ).into());
    }
    let contents = fs::read_to_string(path).map_err(|error| {
        let message = format!("Could not read theme '{}': {error}", theme_file_name(path));
        if error.kind() == ErrorKind::NotFound {
            ThemeFileInspectionError::NotFound(message)
        } else {
            ThemeFileInspectionError::Other(message)
        }
    })?;
    let mut diagnostics = Vec::new();
    let header_result = parse_header(&contents);
    if let Err(error) = &header_result {
        diagnostics.push(ThemeDiagnostic::error(format_source_error(theme_file_name(path), error)));
    }
    let css_errors = validate_theme_css(&contents, theme_file_name(path));
    for (error, level) in css_errors.iter().take(MAX_CSS_DIAGNOSTICS_PER_THEME) {
        diagnostics.push(ThemeDiagnostic {
            message: format_source_error(theme_file_name(path), error),
            level: *level,
        });
    }
    let omitted_css_errors = css_errors.len().saturating_sub(MAX_CSS_DIAGNOSTICS_PER_THEME);
    if omitted_css_errors > 0 {
        diagnostics.push(ThemeDiagnostic::error(format!(
            "{}: {omitted_css_errors} additional CSS diagnostics omitted.",
            theme_file_name(path)
        )));
    }
    let id = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "A custom theme filename is not valid UTF-8.".to_string())?;
    if id.starts_with('.')
        || id.len() > 128
        || !id.to_ascii_lowercase().ends_with(".css")
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b' '))
    {
        diagnostics.push(ThemeDiagnostic::error(format!("{}: A custom theme filename is invalid.", theme_file_name(path))));
        return Ok(ThemeFileLoad { theme: None, diagnostics });
    }
    let theme = header_result.ok().map(|(header, css)| ThemeFile {
        summary: ThemeSummary {
            id: id.to_string(),
            name: header.name,
            author: header.author,
            main_color: theme_main_color(css),
            accent_color: header.accent_color,
            accent_color_selection: header.accent_color_selection,
        },
        css: css.to_string(),
    });
    Ok(ThemeFileLoad { theme, diagnostics })
}

fn theme_file_name(path: &Path) -> &str {
    path.file_name().and_then(|name| name.to_str()).unwrap_or("custom theme")
}

fn theme_file_error(path: &Path, message: &str) -> String {
    let name = theme_file_name(path);
    if message.starts_with(&format!("{name}:")) {
        message.to_string()
    } else {
        format!("{name}: {message}")
    }
}

fn validate_theme_css(contents: &str, file_name: &str) -> Vec<(ThemeSourceError, ThemeDiagnosticLevel)> {
    let warnings = Arc::new(RwLock::new(Vec::new()));
    let options = ParserOptions {
        filename: file_name.to_string(),
        error_recovery: true,
        warnings: Some(Arc::clone(&warnings)),
        ..ParserOptions::default()
    };
    let parsed = StyleSheet::parse(contents, options);
    let mut diagnostics = warnings
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .map(|warning| (css_source_error(warning), ThemeDiagnosticLevel::Warning))
        .collect::<Vec<_>>();
    if let Err(error) = parsed {
        let diagnostic = (css_source_error(&error), ThemeDiagnosticLevel::Error);
        if !diagnostics.iter().any(|(existing, _)| existing == &diagnostic.0) {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

fn css_source_error(error: &lightningcss::error::Error<ParserError<'_>>) -> ThemeSourceError {
    let (line, column) = error
        .loc
        .as_ref()
        .map(|location| (location.line as usize + 1, location.column as usize))
        .unwrap_or((1, 1));
    ThemeSourceError {
        line,
        column,
        message: css_error_message(&error.kind),
    }
}

fn css_error_message(error: &ParserError<'_>) -> String {
    match error {
        ParserError::AtRuleBodyInvalid => "Invalid at-rule body.".into(),
        ParserError::AtRulePreludeInvalid => "Invalid at-rule prelude.".into(),
        ParserError::AtRuleInvalid(name) => format!("Unsupported at-rule `@{name}`."),
        ParserError::EndOfInput => "Unexpected end of file.".into(),
        ParserError::InvalidDeclaration => "Invalid CSS declaration.".into(),
        ParserError::InvalidMediaQuery => "Invalid media query.".into(),
        ParserError::EmptyBracketInCondition => "Empty brackets in CSS condition.".into(),
        ParserError::InvalidNesting => "Invalid CSS nesting.".into(),
        ParserError::DeprecatedNestRule => "The `@nest` rule is deprecated.".into(),
        ParserError::DeprecatedCssModulesValueRule => "The `@value` rule is deprecated.".into(),
        ParserError::InvalidPageSelector => "Invalid page selector.".into(),
        ParserError::InvalidValue => "Invalid CSS value.".into(),
        ParserError::QualifiedRuleInvalid => "Invalid CSS rule.".into(),
        ParserError::SelectorError(_) => "Invalid CSS selector.".into(),
        ParserError::UnexpectedImportRule => "`@import` must appear before other rules.".into(),
        ParserError::UnexpectedNamespaceRule => "`@namespace` must appear before qualified rules.".into(),
        ParserError::UnexpectedToken(token) => {
            let token = shorten_message(&format!("{token:?}"), 48);
            format!("Unexpected CSS token `{token}`.")
        }
        ParserError::MaximumNestingDepth => "CSS nesting is too deep.".into(),
    }
}

fn shorten_message(message: &str, max_chars: usize) -> String {
    let mut chars = message.chars();
    let shortened = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{shortened}...")
    } else {
        shortened
    }
}

fn format_source_error(file_name: &str, error: &ThemeSourceError) -> String {
    format!("{file_name}:{}:{}: {}", error.line, error.column, error.message)
}

fn theme_main_color(css: &str) -> Option<String> {
    let without_comments = Regex::new(r"(?s)/\*.*?\*/")
        .unwrap()
        .replace_all(css, "");
    let declaration = Regex::new(
        r"(?i)(?:^|[;{])\s*--color-bg-primary\s*:\s*(?<color>#[0-9a-f]{3}(?:[0-9a-f]{3})?)\s*(?:!important\s*)?(?:;|})",
    )
    .unwrap();
    declaration
        .captures(&without_comments)
        .and_then(|capture| capture.name("color"))
        .and_then(|color| normalize_theme_color(color.as_str()).ok())
}

fn parse_header(contents: &str) -> Result<(ThemeMetadata, &str), ThemeSourceError> {
    let normalized = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let bom_bytes = contents.len() - normalized.len();
    let trimmed = normalized.trim_start();
    let header_start = bom_bytes + normalized.len() - trimmed.len();
    let Some(header) = trimmed.strip_prefix("/*") else {
        return Err(source_error(
            contents,
            header_start,
            "Expected a `/* ... */` metadata header at the start of the file.",
        ));
    };
    let Some((header, css)) = header.split_once("*/") else {
        return Err(source_error(
            contents,
            header_start,
            "Metadata header is missing its closing `*/`.",
        ));
    };
    let header_offset = header_start + 2;
    let header_end = header_offset + header.len();
    let mut name = None;
    let mut author = None;
    let mut accent_color = None;
    let mut accent_color_selection = None;
    let mut line_offset = header_offset;
    for raw_line in header.split_inclusive('\n') {
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let after_indent = line.trim_start();
        let after_marker = after_indent.strip_prefix('*').unwrap_or(after_indent);
        let normalized_line = after_marker.trim();
        if normalized_line.is_empty() {
            line_offset += raw_line.len();
            continue;
        }
        let Some((key, value)) = normalized_line.split_once(':') else {
            line_offset += raw_line.len();
            continue;
        };
        let key = key.trim();
        let normalized_key = key.to_ascii_lowercase();
        let value = value.trim();
        let value_start = line.find(':').map_or(0, |colon| colon + 1);
        let value_offset = value_start + line[value_start..].len() - line[value_start..].trim_start().len();
        let error_offset = line_offset + value_offset;
        let optional_color_key = normalized_key == "accent-color";
        let based_theme_key = normalized_key == "based-theme";
        let selection_key = matches!(normalized_key.as_str(), "accent-color-selection" | "accent-color-selections");
        let recognized_key = matches!(normalized_key.as_str(), "name" | "author")
            || optional_color_key || based_theme_key || selection_key;
        if !recognized_key {
            line_offset += raw_line.len();
            continue;
        }
        if value.is_empty() {
            return Err(source_error(
                contents,
                error_offset,
                format!("The `{key}` field is empty."),
            ));
        }
        if value.len() > 128 {
            return Err(source_error(
                contents,
                error_offset,
                format!("The `{key}` value exceeds 128 bytes."),
            ));
        }
        if value.chars().any(char::is_control) {
            return Err(source_error(
                contents,
                error_offset,
                format!("The `{key}` value contains a control character."),
            ));
        }
        match normalized_key.as_str() {
            "name" => name = Some(value.to_string()),
            "author" => author = Some(value.to_string()),
            "accent-color" => {
                accent_color = Some(crate::css::normalize_accent(value).map_err(|_| {
                    source_error(contents, error_offset, "Unsupported `accent-color` value.")
                })?);
            }
            "based-theme" => {
                if !value.eq_ignore_ascii_case("dark mode") && !value.eq_ignore_ascii_case("light mode") {
                    return Err(source_error(
                        contents,
                        error_offset,
                        "Expected `Dark Mode` or `Light Mode` for `based-theme`.",
                    ));
                }
            }
            "accent-color-selection" | "accent-color-selections" => {
                accent_color_selection = Some(match value.to_ascii_lowercase().as_str() {
                    "true" => true,
                    "false" => false,
                    _ => {
                        return Err(source_error(
                            contents,
                            error_offset,
                            format!("Expected `true` or `false` for `{key}`."),
                        ));
                    }
                });
            }
            _ => {}
        }
        line_offset += raw_line.len();
    }

    let mut missing = Vec::new();
    if name.is_none() {
        missing.push("`name`");
    }
    if author.is_none() {
        missing.push("`author`");
    }
    if !missing.is_empty() {
        let message = match missing.as_slice() {
            [field] => format!("Theme header is missing required field {field}."),
            _ => "Theme header is missing required fields `name` and `author`.".into(),
        };
        return Err(source_error(contents, header_end, message));
    }
    let name = name.unwrap();
    let author = author.unwrap();
    Ok((ThemeMetadata { name, author, accent_color, accent_color_selection }, css))
}

fn source_error(contents: &str, byte_offset: usize, message: impl Into<String>) -> ThemeSourceError {
    let safe_offset = byte_offset.min(contents.len());
    let prefix = &contents[..safe_offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit_once('\n')
        .map(|(_, after_newline)| after_newline.chars().count() + 1)
        .unwrap_or_else(|| prefix.chars().count() + 1);
    ThemeSourceError { line, column, message: message.into() }
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if parent != path && !parent.exists() {
            ensure_directory(parent)?;
        }
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            reject_reparse_point(&metadata, path)?;
            if !metadata.is_dir() {
                return Err(format!(
                    "Custom theme path is not a directory: {}",
                    path.display()
                ));
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|error| {
                format!(
                    "Could not create custom theme folder {}: {error}",
                    path.display()
                )
            })
        }
        Err(error) => Err(format!(
            "Could not inspect custom theme folder {}: {error}",
            path.display()
        )),
    }
}

fn write_builtin_if_missing(path: &Path, contents: &str, legacy_contents: &str) -> Result<(), String> {
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            std::io::Write::write_all(&mut file, contents.as_bytes()).map_err(|error| {
                format!(
                    "Could not create built-in theme {}: {error}",
                    path.display()
                )
            })
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path).map_err(|error| {
                format!("Could not inspect built-in theme {}: {error}", path.display())
            })?;
            reject_reparse_point(&metadata, path)?;
            if !metadata.is_file() {
                return Ok(());
            }
            let existing = fs::read_to_string(path).map_err(|error| {
                format!("Could not read built-in theme {}: {error}", path.display())
            })?;
            if normalize_newlines(&existing) == normalize_newlines(legacy_contents) {
                fs::write(path, contents).map_err(|error| {
                    format!("Could not update built-in theme {}: {error}", path.display())
                })
            } else {
                Ok(())
            }
        }
        Err(error) => Err(format!(
            "Could not create built-in theme {}: {error}",
            path.display()
        )),
    }
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n")
}

fn reject_reparse_point(metadata: &fs::Metadata, path: &Path) -> Result<(), String> {
    let mut blocked = metadata.file_type().is_symlink();
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        blocked |= metadata.file_attributes() & 0x400 != 0;
    }
    if blocked {
        return Err(format!(
            "Custom themes cannot use a link or reparse point: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_directory(name: &str) -> PathBuf {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        workspace
            .join(".test-run")
            .join(format!("themes-{name}-{}", std::process::id()))
    }

    #[test]
    fn seeds_and_lists_the_builtin_themes_with_author_metadata() {
        let settings = test_directory("catalog").join("Settings");
        let catalog = list_themes(&settings).unwrap();
        let themes = catalog.themes;
        assert!(catalog.diagnostics.is_empty());
        assert!(themes.iter().any(|theme| theme.id == "dark-mode-plus.css"
            && theme.name == "Dark Mode+"
            && theme.author == "PainterFlow11"
            && theme.main_color.as_deref() == Some("#191918")
            && theme.accent_color.is_none()
            && theme.accent_color_selection.is_none()));
        assert!(themes.iter().any(|theme| theme.id == "oled.css"
            && theme.name == "OLED"
            && theme.author == "PainterFlow11"
            && theme.main_color.as_deref() == Some("#000000")
            && theme.accent_color.is_none()
            && theme.accent_color_selection.is_none()));
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn theme_catalog_reports_header_and_css_errors_with_source_locations() {
        let settings = test_directory("diagnostics").join("Settings");
        ensure_builtin_themes(&settings).unwrap();
        let directory = theme_directory(&settings);
        fs::write(
            directory.join("missing-name.css"),
            "/*\nauthor: Example\n*/\n:root { color: red; }",
        )
        .unwrap();
        fs::write(
            directory.join("bad-value.css"),
            "/*\nname: Bad CSS\nauthor: Example\n*/\nbody {\n  color red;\n}\nmain {\n  background blue;\n}",
        )
        .unwrap();

        let catalog = list_themes(&settings).unwrap();
        assert!(!catalog.themes.iter().any(|theme| theme.id == "missing-name.css"));
        assert!(catalog.themes.iter().any(|theme| theme.id == "bad-value.css"));
        assert!(catalog.diagnostics.iter().any(|diagnostic| {
            diagnostic.message == "missing-name.css:3:1: Theme header is missing required field `name`."
                && matches!(diagnostic.level, ThemeDiagnosticLevel::Error)
        }));
        let css_diagnostics = catalog.diagnostics.iter()
            .filter(|diagnostic| diagnostic.message.starts_with("bad-value.css:"))
            .collect::<Vec<_>>();
        assert_eq!(css_diagnostics.len(), 2, "{:?}", catalog.diagnostics);
        assert!(css_diagnostics[0].message.starts_with("bad-value.css:6:")
            && css_diagnostics[0].message.contains("Unexpected CSS token")
            && matches!(css_diagnostics[0].level, ThemeDiagnosticLevel::Warning));
        assert!(css_diagnostics[1].message.starts_with("bad-value.css:9:")
            && css_diagnostics[1].message.contains("Unexpected CSS token")
            && matches!(css_diagnostics[1].level, ThemeDiagnosticLevel::Warning));

        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn theme_catalog_rejects_invalid_based_theme_and_boolean_metadata_values() {
        let settings = test_directory("invalid-theme-metadata").join("Settings");
        ensure_builtin_themes(&settings).unwrap();
        let directory = theme_directory(&settings);
        fs::write(
            directory.join("bad-based-theme.css"),
            "/*\nname: Bad Based Theme\nauthor: Example\nbased-theme: Dwe\n*/\n:root {}",
        )
        .unwrap();
        fs::write(
            directory.join("bad-boolean.css"),
            "/*\nname: Bad Boolean\nauthor: Example\naccent-color: #27A879\naccent-color-selections: tru\n*/\n:root {}",
        )
        .unwrap();

        let catalog = list_themes(&settings).unwrap();
        for (file, expected) in [
            ("bad-based-theme.css", "Expected `Dark Mode` or `Light Mode` for `based-theme`."),
            ("bad-boolean.css", "Expected `true` or `false` for `accent-color-selections`."),
        ] {
            assert!(!catalog.themes.iter().any(|theme| theme.id == file));
            assert!(catalog.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.starts_with(&format!("{file}:"))
                    && diagnostic.message.contains(expected)
                    && matches!(diagnostic.level, ThemeDiagnosticLevel::Error)
            }), "Missing {expected} diagnostic: {:?}", catalog.diagnostics);
        }

        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn missing_selected_themes_are_ignored_but_invalid_files_still_error() {
        let settings = test_directory("selected-read-error").join("Settings");
        ensure_builtin_themes(&settings).unwrap();
        let directory = theme_directory(&settings);
        let missing = directory.join("missing-theme.css");
        assert!(read_theme_file_if_present(&missing).unwrap().is_none());

        let invalid = directory.join("invalid-theme.css");
        fs::write(
            &invalid,
            "/*\nname: Invalid\nauthor: Example\nbased-theme: Dwe\n*/\n:root {}",
        )
        .unwrap();
        let error = read_theme_file_if_present(&invalid).unwrap_err();
        assert!(error.starts_with("invalid-theme.css:") && error.contains("Expected `Dark Mode` or `Light Mode`"), "{error}");
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn theme_catalog_skips_files_that_disappear_before_inspection() {
        let settings = test_directory("catalog-missing-file").join("Settings");
        ensure_builtin_themes(&settings).unwrap();
        let path = theme_directory(&settings).join("deleted.css");
        fs::write(&path, "/*\nname: Deleted\nauthor: Example\n*/\n").unwrap();
        fs::remove_file(&path).unwrap();

        assert!(inspect_theme_file_for_catalog(&path).unwrap().is_none());
        let catalog = list_themes(&settings).unwrap();
        assert!(catalog.diagnostics.is_empty(), "{:?}", catalog.diagnostics);
        assert!(!catalog.themes.iter().any(|theme| theme.id == "deleted.css"));
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn refreshes_stock_builtin_accent_metadata_without_overwriting_local_edits() {
        let settings = test_directory("builtin-accent-migration").join("Settings");
        let builtins = settings.join("themes").join("built-in-themes");
        fs::create_dir_all(&builtins).unwrap();
        fs::write(builtins.join("oled.css"), LEGACY_OLED).unwrap();
        let customized = LEGACY_DARK_MODE_PLUS.replace("--color-bg-primary: #191918;", "--color-bg-primary: #202020;");
        fs::write(builtins.join("dark-mode-plus.css"), &customized).unwrap();

        ensure_builtin_themes(&settings).unwrap();
        assert_eq!(fs::read_to_string(builtins.join("oled.css")).unwrap(), OLED);
        assert_eq!(fs::read_to_string(builtins.join("dark-mode-plus.css")).unwrap(), customized);
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn preset_accent_metadata_is_optional_and_accepts_the_legacy_plural_key() {
        let settings = test_directory("optional-accent").join("Settings");
        ensure_builtin_themes(&settings).unwrap();
        let themes = theme_directory(&settings);
        let bare = themes.join("bare.css");
        fs::write(&bare, "/*\nname: Bare\nauthor: Example\n*/\n").unwrap();
        let summary = read_theme_file_if_present(&bare).unwrap().unwrap().summary;
        assert!(summary.accent_color.is_none());
        assert!(summary.accent_color_selection.is_none());

        let legacy = themes.join("legacy-accent.css");
        fs::write(&legacy, "/*\nname: Legacy\nauthor: Example\naccent-color: #27A879\naccent-color-selections: true\n*/\n").unwrap();
        let summary = read_theme_file_if_present(&legacy).unwrap().unwrap().summary;
        assert_eq!(summary.accent_color.as_deref(), Some("#27A879"));
        assert_eq!(summary.accent_color_selection, Some(true));
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn source_theme_headers_require_name_and_author_but_generated_css_does_not() {
        assert_eq!(
            parse_header("/*\nauthor: Example\n*/\n:root {}")
                .unwrap_err().message,
            "Theme header is missing required field `name`."
        );
        assert_eq!(
            parse_header("/*\nname: \nauthor: Example\n*/\n:root {}")
                .unwrap_err().message,
            "The `name` field is empty."
        );
        assert_eq!(
            parse_header("/*\nname: Example\n*/\n:root {}")
                .unwrap_err().message,
            "Theme header is missing required field `author`."
        );
        assert_eq!(
            parse_header("/*\nname: Example\nauthor: \n*/\n:root {}")
                .unwrap_err().message,
            "The `author` field is empty."
        );

        let settings = test_directory("generated-css-metadata").join("Settings");
        let data = json!({"settings": {
            "enableCustomThemes": true,
            "basedTheme": "dark",
            "themePreset": "",
            "themeColor": "#191918"
        }});
        let generated = render_generated_css(&settings, &data).unwrap();
        assert!(generated.starts_with("/* Generated by BlobUI Custom Themes."));
        let generated_header = generated.split_once("*/").unwrap().0;
        assert!(!generated_header.contains("name:"));
        assert!(!generated_header.contains("author:"));
        assert_eq!(
            parse_header(&generated).unwrap_err().message,
            "Theme header is missing required fields `name` and `author`."
        );
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn extracts_literal_main_colors_and_ignores_comments_or_dynamic_values() {
        assert_eq!(theme_main_color("html { --color-bg-primary: #abc; }"), Some("#AABBCC".into()));
        assert_eq!(theme_main_color("html { --color-bg-primary: #123456 !important; }"), Some("#123456".into()));
        assert_eq!(theme_main_color("/* html { --color-bg-primary: #abc; } */ html { --color-bg-primary: var(--surface); }"), None);
    }

    #[test]
    fn auto_selects_a_palette_and_custom_theme_css_overrides_generated_colors() {
        let settings = test_directory("render").join("Settings");
        ensure_builtin_themes(&settings).unwrap();
        let custom = theme_directory(&settings).join("custom.css");
        fs::write(&custom, "/*\nname: Custom\nauthor: Example\nbased-theme: Light Mode\naccent-color: #123456\naccent-color-selection: false\n*/\n:root { --color-bg-secondary: #123456; }").unwrap();
        let data = json!({"settings": {"enableCustomThemes":true, "basedTheme":"auto", "themePreset":"custom.css", "themeColor":"#191918", "accentColor":"#DE50DE", "includeSelections":true}});
        let css = render_generated_css(&settings, &data).unwrap();
        assert!(css.contains("html { color-scheme: dark; }"));
        assert!(css.contains("--color-bg-primary: #191918;"));
        assert!(css.contains("--color-bg-secondary: #123456;"));
        assert!(css.contains("--color-bg-loader: rgba(0, 0, 0, 0.7);"));
        assert!(css.contains("--color-attachment-loader: rgba(255, 255, 255, 0.7);"));
        assert!(css.contains("--color-text-primary-inversion: #EBEBEB;"));
        assert!(css.contains("--color-grey: #A8A8A8;"));
        assert!(css.contains(".sidebarPage > .bottom .grad { background: linear-gradient(180deg, transparent 3.62%, var(--color-bg-primary) 68.18%); }"));
        assert!(css.contains("html.themeDark #page .socialBlock { background: var(--blobui-discussion-footer-fade-background, linear-gradient(180deg, transparent 3.62%, var(--color-bg-primary) 68.18%)); }"));
        assert!(css.contains("--color-blob-accent: #123456;"));
        assert!(!css.contains("--color-control-accent: var(--color-blob-accent);"));
        assert_eq!(effective_include_selections(&settings, &data).unwrap(), false);
        let selection_off = json!({"settings": {
            "enableCustomThemes":true, "basedTheme":"auto", "themePreset":"", "themeColor":"#191918",
            "accentColor":"#DE50DE", "includeSelections":false
        }});
        let css_without_selections = render_generated_css(&settings, &selection_off).unwrap();
        assert!(css_without_selections.contains("--color-control-accent: #D4D4D4;"));
        assert!(!css_without_selections.contains("--color-control-accent: var(--color-blob-accent);"));
        fs::write(theme_directory(&settings).join("plain.css"), "/*\nname: Plain\nauthor: Example\n*/\n:root { --color-bg-secondary: #123456; }").unwrap();
        let native_accent = json!({"settings": {"enableCustomThemes":true, "basedTheme":"auto", "themePreset":"plain.css", "themeColor":"#191918", "accentColor":"var(--color-system-accent-100)"}});
        let native_css = render_generated_css(&settings, &native_accent).unwrap();
        assert!(native_css.contains("--color-blob-accent: var(--color-system-accent-100);"));
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn dont_override_keeps_light_and_dark_palettes_scoped_to_anytype_mode() {
        let settings = test_directory("inherit").join("Settings");
        let data = json!({"settings": {"enableCustomThemes":true, "basedTheme":"inherit", "themePreset":"", "themeColor":"#abc"}});
        let css = render_generated_css(&settings, &data).unwrap();
        assert!(css.contains("html:not(.themeDark) {"));
        assert!(css.contains("html.themeDark {"));
        assert!(css.contains("--color-bg-primary: #AABBCC;"));
        assert!(!css.contains("color-scheme:"));
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn object_footer_fade_is_controlled_by_the_selected_theme_css() {
        let settings = test_directory("object-footer-fade-theme").join("Settings");
        let oled = json!({"settings": {"enableCustomThemes":true, "basedTheme":"dark", "themePreset":"oled.css", "themeColor":"#000000"}});
        let oled_css = render_generated_css(&settings, &oled).unwrap();
        assert!(oled_css.contains("--blobui-discussion-footer-fade-background: none;"));
        assert!(oled_css.contains("html.themeDark #page .socialBlock { background: var(--blobui-discussion-footer-fade-background, none); }"));
        assert!(!oled_css.contains(".formWrapper .grad"));

        let dark_mode_plus = json!({"settings": {"enableCustomThemes":true, "basedTheme":"dark", "themePreset":"dark-mode-plus.css", "themeColor":"#191918"}});
        let dark_mode_plus_css = render_generated_css(&settings, &dark_mode_plus).unwrap();
        assert!(dark_mode_plus_css.contains("--blobui-discussion-footer-fade-background: linear-gradient(180deg, transparent 3.62%, var(--color-bg-primary) 68.18%);"));
        assert!(dark_mode_plus_css.contains("--color-bg-primary: #191918;"));
        assert!(dark_mode_plus_css.contains("html.themeDark #page .socialBlock { background: var(--blobui-discussion-footer-fade-background, linear-gradient(180deg, transparent 3.62%, var(--color-bg-primary) 68.18%)); }"));
        assert!(!dark_mode_plus_css.contains(".formWrapper .grad"));

        let legacy_settings = test_directory("object-footer-fade-legacy").join("Settings");
        let legacy_themes = theme_directory(&legacy_settings);
        fs::create_dir_all(&legacy_themes).unwrap();
        fs::write(
            legacy_themes.join("oled.css"),
            "/*\nauthor: PainterFlow11\nname: OLED\nbased-theme: Dark Mode\n*/\nhtml { --color-bg-primary: #000000; }",
        )
        .unwrap();
        fs::write(
            legacy_themes.join("dark-mode-plus.css"),
            "/*\nauthor: PainterFlow11\nname: Dark Mode+\nbased-theme: Dark Mode\n*/\nhtml { --color-bg-primary: #191918; }",
        )
        .unwrap();
        let legacy_oled_css = render_generated_css(&legacy_settings, &oled).unwrap();
        assert!(legacy_oled_css.contains("html.themeDark #page .socialBlock { background: var(--blobui-discussion-footer-fade-background, none); }"));
        let legacy_dark_mode_plus_css =
            render_generated_css(&legacy_settings, &dark_mode_plus).unwrap();
        assert!(legacy_dark_mode_plus_css.contains("html.themeDark #page .socialBlock { background: var(--blobui-discussion-footer-fade-background, linear-gradient(180deg, transparent 3.62%, var(--color-bg-primary) 68.18%)); }"));
        let _ = fs::remove_dir_all(settings.parent().unwrap());
        let _ = fs::remove_dir_all(legacy_settings.parent().unwrap());
    }

    #[test]
    fn light_palettes_keep_icons_dark_after_theme_overrides() {
        let settings = test_directory("light-icons").join("Settings");
        ensure_builtin_themes(&settings).unwrap();
        fs::write(
            theme_directory(&settings).join("red-icons.css"),
            "/*\nname: Red Icons\nauthor: Example\n*/\nhtml, html.themeDark { --color-icon: #ff0000; }",
        )
        .unwrap();
        let data = json!({"settings": {"enableCustomThemes":true, "basedTheme":"light", "themePreset":"red-icons.css", "themeColor":"#ffffff"}});
        let css = render_generated_css(&settings, &data).unwrap();
        let custom_icon = css.find("--color-icon: #ff0000;").unwrap();
        let forced_icon = css.rfind("--color-icon: #252525;").unwrap();
        assert!(forced_icon > custom_icon);
        assert!(css.contains("--color-icon-hover: #000000;"));
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn preview_mode_matches_auto_and_inherit_settings() {
        let light = json!({"settings": {"enableCustomThemes":true, "basedTheme":"auto", "themeColor":"#ffffff"}});
        assert_eq!(preview_mode(&light).unwrap(), "light");

        let dark = json!({"settings": {"enableCustomThemes":true, "basedTheme":"auto", "themeColor":"#191918"}});
        assert_eq!(preview_mode(&dark).unwrap(), "dark");

        let inherit = json!({"settings": {"enableCustomThemes":true, "basedTheme":"inherit", "themeColor":"#191918"}});
        assert_eq!(preview_mode(&inherit).unwrap(), "system");
    }

    #[test]
    fn custom_themes_default_off_and_keep_the_users_accent() {
        let settings = test_directory("disabled").join("Settings");
        let data = json!({"settings": {
            "basedTheme":"dark", "themePreset":"oled.css", "themeColor":"#000000",
            "accentColor":"#27A879", "includeSelections":true
        }});
        let css = render_generated_css(&settings, &data).unwrap();
        assert_eq!(preview_mode(&data).unwrap(), "system");
        assert!(!css.contains("--color-bg-primary:"));
        assert!(!css.contains("--blobui-discussion-footer-fade-background:"));
        assert!(css.contains("--color-blob-accent: #27A879;"));
        assert!(css.contains("--color-control-accent: var(--color-blob-accent);"));
        let _ = fs::remove_dir_all(settings.parent().unwrap());
    }

    #[test]
    fn dark_mode_plus_with_auto_uses_a_readable_dark_palette() {
        let settings = test_directory("dark-auto").join("Settings");
        let data = json!({"settings": {
            "enableCustomThemes":true,
            "basedTheme":"auto",
            "themePreset":"dark-mode-plus.css",
            "themeColor":"#191918"
        }});
        let css = render_generated_css(&settings, &data).unwrap();
        assert_eq!(preview_mode(&data).unwrap(), "dark");
        assert!(css.contains("html { color-scheme: dark; }"));
        assert!(css.contains("--color-bg-primary: #191918;"));
        assert!(css.contains("--color-text-primary: #E1E1E1;"));
    }

    #[test]
    fn rejects_invalid_theme_color_and_theme_ids() {
        assert!(normalize_theme_color("red").is_err());
        assert!(selected_theme_path(Path::new("Settings"), "../bad.css").is_err());
    }
}
