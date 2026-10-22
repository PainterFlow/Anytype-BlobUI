use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use url::Url;

const DEFAULT_BANNER: &str = "spacebanner/banner.png";
const MAX_BANNER_BYTES: usize = 2 * 1024 * 1024;

pub fn validate_banner_image(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let prefix = "data:image/";
    let remainder = value
        .strip_prefix(prefix)
        .ok_or_else(|| "The space banner must be a PNG, JPEG, or WebP image.".to_string())?;
    let (format, payload) = remainder
        .split_once(";base64,")
        .ok_or_else(|| "The space banner image data is invalid.".to_string())?;
    if !matches!(format, "png" | "jpeg" | "webp")
        || payload.is_empty()
        || payload.len() > 4 * MAX_BANNER_BYTES.div_ceil(3)
        || !payload
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'='))
    {
        return Err("The space banner must be a PNG, JPEG, or WebP image no larger than 2 MB.".into());
    }
    let bytes = STANDARD
        .decode(payload)
        .map_err(|_| "The space banner image data is invalid.".to_string())?;
    if bytes.is_empty() || bytes.len() > MAX_BANNER_BYTES || STANDARD.encode(&bytes) != payload {
        return Err("The space banner must be a PNG, JPEG, or WebP image no larger than 2 MB.".into());
    }
    let has_expected_signature = match format {
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    };
    if !has_expected_signature {
        return Err("The selected file does not match its PNG, JPEG, or WebP image type.".into());
    }
    Ok(format!("data:image/{format};base64,{payload}"))
}

pub fn default_banner_preview(runtime_root: &Path) -> Result<String, String> {
    let banner = runtime_root.join("assets").join("spacebanner").join("banner.png");
    let bytes = fs::read(&banner)
        .map_err(|error| format!("Could not read the default space banner: {error}"))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

pub fn compile(runtime_root: &Path, install_directory: &Path, data: &Value) -> Result<String, String> {
    let dev_directory = runtime_root.join("Dev");
    let asset_directory = runtime_root.join("assets");
    if !dev_directory.is_dir() {
        return Err(format!("Development CSS directory was not found: {}", dev_directory.display()));
    }
    let banner = asset_directory.join("spacebanner").join("banner.png");
    if !banner.is_file() {
        return Err(format!("The fixed banner image is missing from the packaged assets: {DEFAULT_BANNER}"));
    }

    let mut sources = Vec::new();
    collect_css(&dev_directory, &dev_directory, &mut sources)?;
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    if sources.is_empty() {
        return Err("No CSS source files were found under Dev.".into());
    }

    let newline = newline();
    let mut css_text = String::new();
    for (relative, path) in sources {
        let relative = relative.replace('\\', "/");
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("Could not read CSS source {}: {error}", path.display()))?;
        if !css_text.is_empty() {
            css_text.push_str(newline);
        }
        css_text.push_str(&format!("/* BlobUI source: {relative} */{newline}{source}"));
    }

    let settings = data.get("settings").and_then(Value::as_object)
        .ok_or_else(|| "The settings data has no normalized settings object.".to_string())?;
    let mut bool_settings = ["reducedMotion", "tabHoverEffect", "linkHoverEffect", "objectMentionHoverEffect", "collapsedCategories"]
        .into_iter()
        .map(|name| (name, settings.get(name).and_then(Value::as_bool).unwrap_or(false)))
        .collect::<BTreeMap<_, _>>();
    bool_settings.insert("includeSelections", settings.get("includeSelections").and_then(Value::as_bool).unwrap_or(true));
    let kanban_style = settings.get("kanbanViewStyle").and_then(Value::as_str).unwrap_or("default");
    bool_settings.insert("kanbanBalanced", kanban_style == "balanced");
    bool_settings.insert("kanbanCompact", kanban_style == "compact");
    bool_settings.insert("kanbanStyled", matches!(kanban_style, "balanced" | "compact"));

    let condition = Regex::new(r"(?s)/\*\s*@blobui-if(?<negation>-not)?\s+(?<name>[A-Za-z][A-Za-z0-9]*)\s*\*/(?<content>.*?)/\*\s*@blobui-endif(?<endNegation>-not)?\s+(?<endName>[A-Za-z][A-Za-z0-9]*)\s*\*/").unwrap();
    let mut condition_error = None;
    css_text = condition.replace_all(&css_text, |captures: &regex::Captures<'_>| {
        let name = captures.name("name").unwrap().as_str();
        if captures.name("endName").unwrap().as_str() != name
            || captures.name("negation").map(|value| value.as_str())
                != captures.name("endNegation").map(|value| value.as_str())
        {
            condition_error = Some(format!("The BlobUI CSS condition for '{name}' has a mismatched closing marker."));
            return String::new();
        }
        let Some(enabled) = bool_settings.get(name) else {
            condition_error = Some(format!("The BlobUI CSS condition references an unknown setting: {name}"));
            return String::new();
        };
        let is_negated = captures.name("negation").is_some();
        if *enabled != is_negated { captures.name("content").unwrap().as_str().to_string() } else { String::new() }
    }).into_owned();
    if let Some(error) = condition_error {
        return Err(error);
    }
    let unmatched_condition = Regex::new(r"/\*\s*@blobui-(?:if(?:-not)?|endif(?:-not)?)\s+[A-Za-z]").unwrap();
    if unmatched_condition.is_match(&css_text) {
        return Err("CSS contains an unmatched accessibility setting condition.".into());
    }

    let declarations = placeholder_declarations(&css_text)?;
    let install_directory_text = data.get("installDirectory").and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "The settings data has no install directory.".to_string())?;
    if !same_install_directory(Path::new(install_directory_text), install_directory) {
        return Err("The saved settings directory does not match the selected Anytype directory.".into());
    }
    let banner_target = install_directory.join("blob").join("assets").join(DEFAULT_BANNER.replace('/', std::path::MAIN_SEPARATOR_STR));
    let banner_image = settings
        .get("bannerImage")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let banner_uri = if banner_image.is_empty() {
        Url::from_file_path(&banner_target)
            .map_err(|_| "The banner path could not be converted to a local file URI.".to_string())?
            .to_string()
    } else {
        validate_banner_image(banner_image)?
    };
    let values: BTreeMap<&str, (&str, String)> = BTreeMap::from([
        ("fontFamily", ("font", setting_string(settings, "fontFamily")?.to_string())),
        ("codeFontFamily", ("font", setting_string(settings, "codeFontFamily")?.to_string())),
        ("accentColor", ("color", normalize_accent(setting_string(settings, "accentColor")?)?)),
        ("bannerUrl", ("url", banner_uri)),
    ]);
    let placeholder = Regex::new(r"\{\{\s*(?<name>[A-Za-z][A-Za-z0-9]*)\s*\}\}").unwrap();
    for capture in placeholder.captures_iter(&css_text) {
        let name = capture.name("name").unwrap().as_str();
        let Some((value_type, _)) = values.get(name) else {
            return Err(format!("The '{name}' CSS placeholder has no settings value."));
        };
        if declarations.get(name).map(String::as_str) != Some(*value_type) {
            return Err(format!("The '{name}' CSS placeholder type does not match its settings value."));
        }
    }

    css_text = placeholder.replace_all(&css_text, |captures: &regex::Captures<'_>| {
        let name = captures.name("name").unwrap().as_str();
        values.get(name).map(|(_, value)| value.as_str()).unwrap_or_default().to_string()
    }).into_owned();

    let accent = normalize_accent(setting_string(settings, "accentColor")?)?;
    let custom_accent = accent != "var(--color-system-accent-100)";
    let custom_condition = Regex::new(r"(?s)/\*\s*@blobui-if-custom-accent\s*\*/(?<content>.*?)/\*\s*@blobui-endif-custom-accent\s*\*/").unwrap();
    css_text = custom_condition.replace_all(&css_text, |captures: &regex::Captures<'_>| {
        if custom_accent { captures.name("content").unwrap().as_str().to_string() } else { String::new() }
    }).into_owned();
    if Regex::new(r"/\*\s*@blobui-(?:if-custom-accent|endif-custom-accent)\s*\*/").unwrap().is_match(&css_text) {
        return Err("CSS contains an unmatched custom accent condition.".into());
    }

    let declaration = Regex::new(r"/\*\s*@blobui-placeholder\s+[A-Za-z][A-Za-z0-9]*\s+(?:font|color|url|number|boolean)\s*\*/").unwrap();
    css_text = declaration.replace_all(&css_text, "").into_owned();
    let api_disabled = data.pointer("/apiConnection/enabled").and_then(Value::as_bool) == Some(false);
    if !api_disabled && settings.get("calendarColorsEnabled").and_then(Value::as_bool).unwrap_or(true) {
        if let Some(calendar_css) = calendar_color_css(data.get("calendarObjectColors"))? {
            if !css_text.trim_end().is_empty() {
                css_text = format!("{}{newline}{newline}{calendar_css}{newline}", css_text.trim_end());
            }
        }
    }
    if Regex::new(r"\{\{[^}]+\}\}").unwrap().is_match(&css_text) {
        return Err("Compiled CSS still contains an unresolved placeholder.".into());
    }
    Ok(css_text)
}

fn same_install_directory(saved: &Path, selected: &Path) -> bool {
    #[cfg(windows)]
    {
        comparable_windows_path(saved).eq_ignore_ascii_case(&comparable_windows_path(selected))
    }
    #[cfg(not(windows))]
    {
        saved == selected
    }
}

fn comparable_windows_path(path: &Path) -> String {
    const VERBATIM_PREFIX: &str = "\\\\?\\";
    const VERBATIM_UNC_PREFIX: &str = "\\\\?\\UNC\\";

    let path = path.to_string_lossy().replace('/', "\\");
    if path
        .get(..VERBATIM_UNC_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(VERBATIM_UNC_PREFIX))
    {
        return format!(r"\\{}", &path[VERBATIM_UNC_PREFIX.len()..]);
    }
    if path
        .get(..VERBATIM_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(VERBATIM_PREFIX))
    {
        let without_prefix = &path[VERBATIM_PREFIX.len()..];
        if without_prefix
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && without_prefix.as_bytes().get(1) == Some(&b':')
        {
            return without_prefix.to_owned();
        }
    }
    path
}

fn collect_css(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    let entries = fs::read_dir(directory).map_err(|error| format!("Could not read CSS directory {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Could not inspect CSS files: {error}"))?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|error| format!("Could not inspect {}: {error}", entry.path().display()))?;
        if metadata.file_type().is_symlink() { continue; }
        if metadata.is_dir() {
            collect_css(root, &entry.path(), files)?;
        } else if metadata.is_file() && entry.path().extension().is_some_and(|extension| extension.eq_ignore_ascii_case("css")) {
            let relative = entry.path().strip_prefix(root).unwrap_or(&entry.path()).to_string_lossy().replace('\\', "/");
            files.push((relative, entry.path()));
        }
    }
    Ok(())
}

fn placeholder_declarations(css: &str) -> Result<BTreeMap<String, String>, String> {
    let declaration = Regex::new(r"/\*\s*@blobui-placeholder\s+(?<name>[A-Za-z][A-Za-z0-9]*)\s+(?<type>font|color|url|number|boolean)\s*\*/").unwrap();
    let mut declarations = BTreeMap::new();
    for capture in declaration.captures_iter(css) {
        let name = capture.name("name").unwrap().as_str().to_string();
        let value_type = capture.name("type").unwrap().as_str().to_string();
        if declarations.get(&name).is_some_and(|existing| existing != &value_type) {
            return Err(format!("The '{name}' CSS placeholder has conflicting type declarations."));
        }
        declarations.insert(name, value_type);
    }
    if declarations.is_empty() && css.contains("{{") {
        return Err("CSS placeholders must have explicit @blobui-placeholder declarations.".into());
    }
    Ok(declarations)
}

fn setting_string<'a>(settings: &'a serde_json::Map<String, Value>, name: &str) -> Result<&'a str, String> {
    settings.get(name).and_then(Value::as_str).ok_or_else(|| format!("The '{name}' setting has no text value."))
}

pub fn normalize_accent(value: &str) -> Result<String, String> {
    if value.trim().eq_ignore_ascii_case("default") || value.trim() == "var(--color-system-accent-100)" {
        return Ok("var(--color-system-accent-100)".into());
    }
    color_css(value, "Accent Color", false, true)
}

pub fn validate_calendar_color(value: &str) -> Result<(), String> {
    color_css(value, "Calendar Color", true, false).map(|_| ())
}

fn color_css(value: &str, name: &str, allow_default: bool, reject_white: bool) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 {
        return Err(format!("'{name}' must be a supported color value."));
    }
    let lower = value.to_ascii_lowercase();
    let palettes = [
        ("grey", "Grey", "var(--color-grey)"), ("yellow", "Yellow", "var(--color-yellow)"),
        ("amber", "Amber", "var(--color-amber, var(--color-orange))"), ("red", "Red", "var(--color-red)"),
        ("pink", "Pink", "var(--color-pink)"), ("purple", "Purple", "var(--color-purple)"),
        ("blue", "Blue", "var(--color-blue)"), ("sky", "Sky", "var(--color-sky, var(--color-ice))"),
        ("teal", "Teal", "var(--color-teal)"), ("green", "Green", "var(--color-green, var(--color-lime))"),
    ];
    if lower == "default" {
        return if allow_default { Ok(String::new()) } else { Err(format!("'{name}' does not accept Default.")) };
    }
    if let Some((_, canonical, css)) = palettes.iter().find(|(key, _, _)| *key == lower) {
        let _ = canonical;
        return Ok(css.to_string());
    }
    if lower == "blob pink" { return Ok("#DE50DE".into()); }
    if lower == "white" {
        if reject_white { return Err(format!("White is not supported for {name}. Choose another color or enter 'default'.")); }
        return Ok("#FFFFFF".into());
    }
    let variables = [
        "var(--color-grey)", "var(--color-yellow)", "var(--color-amber, var(--color-orange))",
        "var(--color-red)", "var(--color-pink)", "var(--color-purple)", "var(--color-blue)",
        "var(--color-sky, var(--color-ice))", "var(--color-teal)", "var(--color-green, var(--color-lime))",
    ];
    if variables.contains(&value) { return Ok(value.into()); }
    if value.starts_with('#') {
        let digits = &value[1..];
        if (digits.len() == 3 || digits.len() == 6) && digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            if reject_white && (digits.eq_ignore_ascii_case("fff") || digits.eq_ignore_ascii_case("ffffff")) {
                return Err(format!("White is not supported for {name}. Choose another color or enter 'default'."));
            }
            return Ok(value.to_ascii_uppercase());
        }
    }
    if let Some(body) = rgb_body(value) {
        let (channels, alpha, percent_channels) = parse_rgb_components(body, name)?;
        if reject_white && channels.iter().all(|channel| *channel == if percent_channels { 100.0 } else { 255.0 }) {
            return Err(format!("White is not supported for {name}. Choose another color or enter 'default'."));
        }
        if let Some(alpha) = alpha {
            let maximum = if alpha.ends_with('%') { 100.0 } else { 1.0 };
            let alpha_value = numeric_component(alpha).ok_or_else(|| format!("'{name}' contains an rgb()/rgba() alpha outside its valid range."))?;
            if alpha_value > maximum { return Err(format!("'{name}' contains an rgb()/rgba() alpha outside its valid range.")); }
        }
        return Ok(value.into());
    }
    Err(format!("'{name}' must be a three or six digit hexadecimal color, a valid rgb()/rgba() color, an Anytype color name (Grey, Yellow, Amber, Red, Pink, Purple, Blue, Sky, Teal, or Green), or Blob Pink."))
}

fn rgb_body(value: &str) -> Option<&str> {
    let open = value.find('(')?;
    if !value[..open].eq_ignore_ascii_case("rgb") && !value[..open].eq_ignore_ascii_case("rgba") { return None; }
    value.strip_suffix(')')?.get(open + 1..)
}

fn parse_rgb_components<'a>(body: &'a str, name: &str) -> Result<(Vec<f64>, Option<&'a str>, bool), String> {
    let invalid = || format!("'{name}' contains an invalid rgb()/rgba() color.");
    let (channels, alpha): (Vec<&str>, Option<&str>) = if body.contains(',') {
        if body.contains('/') { return Err(invalid()); }
        let parts = body.split(',').map(str::trim).collect::<Vec<_>>();
        if !(parts.len() == 3 || parts.len() == 4) { return Err(invalid()); }
        (parts[..3].to_vec(), parts.get(3).copied())
    } else {
        let mut halves = body.split('/');
        let channel_text = halves.next().unwrap_or_default();
        let alpha = halves.next().map(str::trim);
        if halves.next().is_some() { return Err(invalid()); }
        let channels = channel_text.split_whitespace().collect::<Vec<_>>();
        if channels.len() != 3 { return Err(invalid()); }
        (channels, alpha)
    };
    let percent_count = channels.iter().filter(|channel| channel.ends_with('%')).count();
    if percent_count != 0 && percent_count != 3 { return Err(format!("'{name}' contains mixed rgb()/rgba() channel units.")); }
    let percent_channels = percent_count == 3;
    let limit = if percent_channels { 100.0 } else { 255.0 };
    let mut values = Vec::with_capacity(3);
    for channel in channels {
        let value = numeric_component(channel).ok_or_else(invalid)?;
        if value > limit { return Err(format!("'{name}' contains an rgb()/rgba() channel outside its valid range.")); }
        values.push(value);
    }
    if let Some(alpha) = alpha {
        let maximum = if alpha.ends_with('%') { 100.0 } else { 1.0 };
        let value = numeric_component(alpha).ok_or_else(|| format!("'{name}' contains an rgb()/rgba() alpha outside its valid range."))?;
        if value > maximum { return Err(format!("'{name}' contains an rgb()/rgba() alpha outside its valid range.")); }
    }
    Ok((values, alpha, percent_channels))
}

fn numeric_component(value: &str) -> Option<f64> {
    let number = value.trim().strip_suffix('%').unwrap_or(value.trim()).parse::<f64>().ok()?;
    (number.is_finite() && number >= 0.0).then_some(number)
}

fn calendar_color_css(colors: Option<&Value>) -> Result<Option<String>, String> {
    let Some(colors) = colors.and_then(Value::as_object) else { return Ok(None); };
    let newline = newline();
    let mut rules = Vec::new();
    for (object_id, raw_color) in colors {
        if !Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$").unwrap().is_match(object_id) {
            return Err(format!("Calendar Color data contains an invalid Anytype object ID: {object_id}"));
        }
        let raw = raw_color.as_str().ok_or_else(|| format!("Calendar Color data for '{object_id}' is not text."))?;
        let color = color_css(raw, "Calendar Color", true, false)?;
        if color.is_empty() { continue; }
        let selector = format!("[data-view-type=\"4\"][data-drop-type=\"record\"][data-id=\"{object_id}\"]:not(.record *),");
        let calendar = format!(".viewContent.viewCalendar .record[id=\"record-{object_id}\"]");
        rules.push(selector);
        rules.push(format!("{calendar} {{"));
        rules.push(format!("    background-color: {color} !important;"));
        rules.push(format!("    background-color: color-mix(in srgb, {color} 18%, #1b1b1b) !important;"));
        rules.push(format!("    border-left: 3px solid {color} !important;"));
        rules.push("    border-radius: 3px !important;".into());
        rules.push("    color: #FFFFFF !important;".into());
        rules.push("}".into());
        rules.push(format!("[data-view-type=\"4\"][data-drop-type=\"record\"][data-id=\"{object_id}\"] .name,"));
        rules.push(format!("[data-view-type=\"4\"][data-drop-type=\"record\"][data-id=\"{object_id}\"] .name span,"));
        rules.push(format!("{calendar} .name,"));
        rules.push(format!("{calendar} .name span {{"));
        rules.push("    color: #FFFFFF !important;".into());
        rules.push("}".into());
        let menu = format!(".content > .wrap > .items > .item[id=\"item-{object_id}\"]:has(> .iconObject[id=\"menuCalendarDay-{object_id}-icon\"])");
        rules.push(format!("{menu} {{"));
        rules.push(format!("    background-color: {color} !important;"));
        rules.push(format!("    background-color: color-mix(in srgb, {color} 18%, #1b1b1b) !important;"));
        rules.push(format!("    border-left: 3px solid {color} !important;"));
        rules.push("    border-radius: 3px !important;".into());
        rules.push("    color: #FFFFFF !important;".into());
        rules.push("}".into());
        rules.push(format!("{menu} .name,"));
        rules.push(format!("{menu} .name span {{"));
        rules.push("    color: #FFFFFF !important;".into());
        rules.push("}".into());
    }
    if rules.is_empty() { Ok(None) } else { Ok(Some(format!("/* BlobUI Calendar Color overrides */{newline}{}", rules.join(newline)))) }
}

#[cfg(windows)]
fn newline() -> &'static str { "\r\n" }
#[cfg(not(windows))]
fn newline() -> &'static str { "\n" }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(".test-run");
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        root.join(format!("css-{}-{unique}", std::process::id()))
    }

    #[test]
    #[cfg(windows)]
    fn accepts_equivalent_dos_and_verbatim_install_paths() {
        let root = fixture();
        fs::create_dir_all(root.join("Dev")).unwrap();
        fs::create_dir_all(root.join("assets/spacebanner")).unwrap();
        fs::write(root.join("assets/spacebanner/banner.png"), b"fixture").unwrap();
        fs::write(root.join("Dev/00-test.css"), "body {}").unwrap();
        let install = root.join("anytype");
        let selected = install.to_string_lossy();
        let verbatim = if selected.starts_with(r"\\") {
            format!(r"\\?\UNC\{}", selected.trim_start_matches('\\'))
        } else {
            format!(r"\\?\{selected}")
        };
        let data = json!({
            "installDirectory": verbatim,
            "settings": {
                "fontFamily": "Inter",
                "codeFontFamily": "Consolas",
                "accentColor": "Default"
            }
        });

        assert!(compile(&root, &install, &data).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compiles_all_kanban_combinations_and_defaults_without_overrides() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let root = fixture();
        fs::create_dir_all(root.join("Dev")).unwrap();
        fs::create_dir_all(root.join("assets/spacebanner")).unwrap();
        fs::write(root.join("assets/spacebanner/banner.png"), b"fixture").unwrap();
        fs::copy(workspace.join("Dev/41-kanban.css"), root.join("Dev/41-kanban.css")).unwrap();
        for style in ["default", "balanced", "compact", "invalid"] {
            for collapsed in [false, true] {
                let data = json!({"installDirectory": root.join("anytype"), "settings": {
                    "fontFamily": "Inter", "codeFontFamily": "Consolas", "accentColor": "Default",
                    "kanbanViewStyle": style, "collapsedCategories": collapsed
                }});
                let compiled = compile(&root, &root.join("anytype"), &data).unwrap();
                assert!(!compiled.contains("@blobui-if"));
                assert_eq!(compiled.contains("column-gap: 8px"), matches!(style, "balanced" | "compact"));
                assert_eq!(compiled.contains("flex-wrap: wrap"), style == "compact");
                assert_eq!(compiled.contains(":focus-within"), collapsed);
                if matches!(style, "default" | "invalid") && !collapsed {
                    assert!(!compiled.contains("#columns"));
                }
            }
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compiles_local_settings_placeholders_and_calendar_colors() {
        let root = fixture();
        fs::create_dir_all(root.join("Dev/nested")).unwrap();
        fs::create_dir_all(root.join("assets/spacebanner")).unwrap();
        fs::write(root.join("assets/spacebanner/banner.png"), b"fixture").unwrap();
        fs::write(root.join("Dev/10-test.css"), "/* @blobui-placeholder fontFamily font */\n/* @blobui-placeholder bannerUrl url */\nbody { font-family: \"{{fontFamily}}\"; --header-image: url(\"{{bannerUrl}}\"); }\n/* @blobui-if tabHoverEffect */\n.tab {}\n/* @blobui-endif tabHoverEffect */").unwrap();
        fs::write(root.join("Dev/nested/00-accent.css"), "/* @blobui-placeholder accentColor color */\n:root { color: {{accentColor}}; }").unwrap();
        let install = root.join("anytype");
        let mut data = json!({"installDirectory": install, "settings": {
            "fontFamily": "Inter", "codeFontFamily": "Consolas", "accentColor": "#27A879", "tabHoverEffect": true
        }, "calendarObjectColors": {"event-1": "Green"}});
        let css = compile(&root, &install, &data).unwrap();
        assert!(css.contains("font-family: \"Inter\""));
        assert!(css.contains("color: #27A879"));
        assert!(css.contains(".tab {}"));
        assert!(css.contains("record-event-1"));
        data["settings"]["calendarColorsEnabled"] = json!(false);
        assert!(!compile(&root, &install, &data).unwrap().contains("record-event-1"));
        assert_eq!(data["calendarObjectColors"]["event-1"], "Green");
        data["settings"]["calendarColorsEnabled"] = json!(true);
        assert!(compile(&root, &install, &data).unwrap().contains("record-event-1"));
        assert!(!css.contains("@blobui-placeholder"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn validates_and_compiles_custom_banner_data_urls() {
        let bytes = b"\x89PNG\r\n\x1a\n";
        let data_url = format!("data:image/png;base64,{}", STANDARD.encode(bytes));
        assert_eq!(validate_banner_image(&data_url).unwrap(), data_url);
        assert!(validate_banner_image("data:image/png;base64,Zm9v").is_err());
        assert!(validate_banner_image("data:image/svg+xml;base64,Zm9v").is_err());

        let root = fixture();
        fs::create_dir_all(root.join("Dev")).unwrap();
        fs::create_dir_all(root.join("assets/spacebanner")).unwrap();
        fs::write(root.join("assets/spacebanner/banner.png"), b"fixture").unwrap();
        fs::write(root.join("Dev/00-banner.css"), "/* @blobui-placeholder bannerUrl url */\n:root { --header-image: url(\"{{bannerUrl}}\"); }").unwrap();
        let install = root.join("anytype");
        let data = json!({"installDirectory": install, "settings": {
            "fontFamily": "Inter", "codeFontFamily": "Consolas", "accentColor": "#27A879", "bannerImage": data_url
        }});
        let css = compile(&root, &install, &data).unwrap();
        assert!(css.contains(&format!("url(\"{data_url}\")")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_undeclared_and_unsafe_calendar_values() {
        assert!(color_css("rgb(256, 0, 0)", "Calendar Color", false, false).is_err());
        assert!(color_css("#FFFFFF", "Accent Color", false, true).is_err());
        let invalid = json!({"event/unsafe": "Green"});
        assert!(calendar_color_css(invalid.as_object().map(|_| &invalid)).is_err());
    }

    #[test]
    fn selection_css_defaults_on_and_compiles_independently_of_accent_mode() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let fixture = root.join(".test-run/selections-compiled");
        fs::create_dir_all(&fixture).unwrap();
        let install = fixture.join("Anytype");
        for (name, accent) in [("native", "Default"), ("preset", "Blob Pink"), ("custom", "#27A879"), ("alpha", "rgba(12, 34, 56, 0.5)")] {
            for enabled in [None, Some(false), Some(true)] {
                let mut data = serde_json::json!({"installDirectory": install, "settings": {
                    "fontFamily": "Inter", "codeFontFamily": "Consolas", "accentColor": accent
                }});
                if let Some(value) = enabled { data["settings"]["includeSelections"] = serde_json::json!(value); }
                let css = compile(root, &install, &data).unwrap();
                assert_eq!(css.contains("--blobui-selection-fill:"), enabled.unwrap_or(true));
                assert!(!css.contains("@blobui-"));
                assert!(!css.contains("{{"));
                if let Some(value) = enabled {
                    fs::write(fixture.join(format!("{name}-{value}.css")), css).unwrap();
                }
            }
        }
    }

    #[test]
    fn compiles_repository_theme_for_accent_accessibility_and_calendar_settings() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let fixture = fixture().join("parity");
        fs::create_dir_all(&fixture).unwrap();
        let install = fixture.join("Anytype data");
        let cases = [
            json!({
                "settings": { "fontFamily": "Inter", "codeFontFamily": "Cascadia Code", "accentColor": "Default", "reducedMotion": false, "tabHoverEffect": false, "linkHoverEffect": false, "objectMentionHoverEffect": false },
                "calendarObjectColors": { "default-case": "Green" }
            }),
            json!({
                "settings": { "fontFamily": "Libron", "codeFontFamily": "Consolas", "accentColor": "#27A879", "reducedMotion": true, "tabHoverEffect": true, "linkHoverEffect": false, "objectMentionHoverEffect": true },
                "calendarObjectColors": { "custom-case": "Purple" }
            }),
            json!({
                "settings": { "fontFamily": "Arial", "codeFontFamily": "Consolas", "accentColor": "#3656D4", "reducedMotion": false, "tabHoverEffect": true, "linkHoverEffect": true, "objectMentionHoverEffect": false },
                "apiConnection": { "enabled": false },
                "calendarObjectColors": { "disabled-case": "Amber" }
            }),
            json!({
                "settings": { "fontFamily": "Arial", "codeFontFamily": "Consolas", "accentColor": "#3656D4", "bannerImage": "data:image/png;base64,iVBORw0KGgo=", "reducedMotion": false, "tabHoverEffect": true, "linkHoverEffect": true, "objectMentionHoverEffect": true },
                "apiConnection": { "enabled": false },
                "calendarObjectColors": {}
            }),
        ];
        for (index, settings) in cases.iter().enumerate() {
            let mut data = settings.clone();
            data["installDirectory"] = json!(install);
            let compiled = compile(root, &install, &data).unwrap();
            assert!(!compiled.contains("{{"), "unresolved setting in case {index}");
            assert!(!compiled.contains("@blobui-"), "unresolved directive in case {index}");
            assert!(compiled.contains(settings["settings"]["fontFamily"].as_str().unwrap()));
            assert_eq!(
                compiled.contains("#preview.previewWrapper:has(.previewLink)"),
                !settings["settings"]["linkHoverEffect"].as_bool().unwrap(),
                "link preview toggle was not compiled independently in case {index}"
            );
            assert_eq!(
                compiled.contains("#preview.previewWrapper:has(.previewDefault, .previewObject)"),
                !settings["settings"]["objectMentionHoverEffect"].as_bool().unwrap(),
                "object mention preview toggle was not compiled independently in case {index}"
            );
            if index == 0 {
                assert!(compiled.contains("record-default-case"));
            } else if index == 1 {
                assert!(compiled.contains("record-custom-case"));
                assert!(compiled.contains("#27A879"));
            } else {
                assert!(!compiled.contains("record-disabled-case"));
            }
            if index == 3 {
                assert!(compiled.contains("data:image/png;base64,iVBORw0KGgo="));
            }
        }
        fs::remove_dir_all(fixture).unwrap();
    }
}
