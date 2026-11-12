use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const MAX_THEME_BYTES: u64 = 1024 * 1024;
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

#[derive(Debug, Clone)]
struct ThemeFile {
    summary: ThemeSummary,
    css: String,
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

pub(crate) fn list_themes(settings_directory: &Path) -> Result<Vec<ThemeSummary>, String> {
    ensure_builtin_themes(settings_directory)?;
    let directory = theme_directory(settings_directory);
    let entries = fs::read_dir(&directory).map_err(|error| {
        format!(
            "Could not read custom themes from {}: {error}",
            directory.display()
        )
    })?;
    let mut themes = Vec::new();
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
        let Ok(theme) = read_theme_file(&path) else {
            continue;
        };
        themes.push(theme.summary);
    }
    themes.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then(left.id.cmp(&right.id))
    });
    Ok(themes)
}

pub(crate) fn render_generated_css(
    settings_directory: &Path,
    data: &Value,
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
    let include_selections = effective_include_selections(settings_directory, data)?;
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
        Some(read_theme_file(&path)?)
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
    let theme = read_theme_file(&selected_theme_path(settings_directory, theme_id)?)?;
    Ok(theme.summary.accent_color_selection.unwrap_or(user_setting))
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

fn read_theme_file(path: &Path) -> Result<ThemeFile, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not read theme file {}: {error}", path.display()))?;
    reject_reparse_point(&metadata, path)?;
    if !metadata.is_file() || metadata.len() > MAX_THEME_BYTES {
        return Err(format!(
            "Theme file {} must be a CSS file no larger than 1 MB.",
            path.display()
        ));
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Could not read theme file {}: {error}", path.display()))?;
    let (header, css) = parse_header(&contents)?;
    let id = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("A custom theme filename is not valid UTF-8.")?;
    if id.starts_with('.')
        || id.len() > 128
        || !id.to_ascii_lowercase().ends_with(".css")
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b' '))
    {
        return Err("A custom theme filename is invalid.".into());
    }
    Ok(ThemeFile {
        summary: ThemeSummary {
            id: id.to_string(),
            name: header.name,
            author: header.author,
            main_color: theme_main_color(css),
            accent_color: header.accent_color,
            accent_color_selection: header.accent_color_selection,
        },
        css: css.to_string(),
    })
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

fn parse_header(contents: &str) -> Result<(ThemeMetadata, &str), String> {
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let trimmed = contents.trim_start();
    let Some(header) = trimmed.strip_prefix("/*") else {
        return Err("A custom theme needs a commented metadata header.".into());
    };
    let Some((header, css)) = header.split_once("*/") else {
        return Err("A custom theme metadata header is not closed.".into());
    };
    let mut name = None;
    let mut author = None;
    let mut accent_color = None;
    let mut accent_color_selection = None;
    for line in header.lines() {
        let line = line.trim().trim_start_matches('*').trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.len() > 128 || value.chars().any(char::is_control) {
            continue;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            "name" if !value.is_empty() => name = Some(value.to_string()),
            "author" if !value.is_empty() => author = Some(value.to_string()),
            "accent-color" if !value.is_empty() => {
                accent_color = Some(crate::css::normalize_accent(value)?);
            }
            "accent-color-selection" | "accent-color-selections" => {
                accent_color_selection = Some(match value.to_ascii_lowercase().as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err("The accent-color-selection theme option must be true or false.".into()),
                });
            }
            _ => {}
        }
    }
    let name = name.ok_or("A custom theme header must include a name.")?;
    let author = author.ok_or("A custom theme header must include an author.")?;
    Ok((ThemeMetadata { name, author, accent_color, accent_color_selection }, css))
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
        let themes = list_themes(&settings).unwrap();
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
        let summary = read_theme_file(&bare).unwrap().summary;
        assert!(summary.accent_color.is_none());
        assert!(summary.accent_color_selection.is_none());

        let legacy = themes.join("legacy-accent.css");
        fs::write(&legacy, "/*\nname: Legacy\nauthor: Example\naccent-color: #27A879\naccent-color-selections: true\n*/\n").unwrap();
        let summary = read_theme_file(&legacy).unwrap().summary;
        assert_eq!(summary.accent_color.as_deref(), Some("#27A879"));
        assert_eq!(summary.accent_color_selection, Some(true));
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
