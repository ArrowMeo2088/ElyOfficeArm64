//! The app's theme as CSS variables on the page's root.

use ely_gpui_component::theme::{ActiveTheme as _, Palette};
use gpui::{App, Hsla, Rgba};

fn css(color: Hsla) -> String {
    let rgba = Rgba::from(color);
    let byte = |part: f32| (part.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "rgba({}, {}, {}, {:.3})",
        byte(rgba.r),
        byte(rgba.g),
        byte(rgba.b),
        rgba.a
    )
}

/// `--ely-*` names and values, palette then type.
pub fn tokens(cx: &App) -> Vec<(String, String)> {
    let theme = cx.theme();
    let colors: &Palette = &theme.colors;
    let mut out: Vec<(String, String)> = [
        ("bg", colors.bg),
        ("surface", colors.surface),
        ("sunken", colors.sunken),
        ("hover", colors.hover),
        ("border", colors.border),
        ("fg", colors.fg),
        ("fg-muted", colors.fg_muted),
        ("fg-subtle", colors.fg_subtle),
        ("accent", colors.accent),
        ("on-accent", colors.on_accent),
        ("focus", colors.focus),
        ("link", colors.link),
        ("selection", colors.selection),
        ("danger", colors.danger),
        ("warning", colors.warning),
        ("success", colors.success),
        ("keyword", colors.syntax.keyword),
        ("string", colors.syntax.string),
        ("number", colors.syntax.number),
        ("comment", colors.syntax.comment),
        ("function", colors.syntax.function),
        ("type", colors.syntax.type_name),
        ("tag", colors.syntax.tag),
        ("attribute", colors.syntax.attribute),
        ("punctuation", colors.syntax.punctuation),
    ]
    .into_iter()
    .map(|(name, color)| (format!("--ely-{name}"), css(color)))
    .collect();
    out.push(("--ely-font".into(), format!("\"{}\"", theme.font_family)));
    out.push(("--ely-mono".into(), format!("\"{}\"", theme.mono_family)));
    out.push(("--ely-font-scale".into(), theme.font_scale.to_string()));
    let scheme = if theme.is_dark() { "dark" } else { "light" };
    out.push(("color-scheme".into(), scheme.into()));
    out
}

/// Script that sets `tokens` on `:root`.
pub fn script(tokens: &[(String, String)]) -> String {
    let pairs = serde_json::to_string(tokens).expect("strings serialize");
    format!("for (const [k, v] of {pairs}) document.documentElement.style.setProperty(k, v);")
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_script_sets_each_token_as_json() {
        let tokens = vec![("--ely-fg".to_string(), "rgba(1, 2, 3, 1.000)".to_string())];
        let script = super::script(&tokens);
        assert!(
            script.contains(r#"[["--ely-fg","rgba(1, 2, 3, 1.000)"]]"#),
            "{script}"
        );
    }
}
