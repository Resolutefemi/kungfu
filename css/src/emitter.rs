//! CSS emitter — turn a list of `ClassToken`s into actual CSS rules.
//!
//! The emitter knows the mapping from utility name → CSS declaration block.
//! It groups rules by responsive prefix and emits `@media` queries for
//! responsive variants.

use std::collections::{BTreeMap, HashMap};

use crate::parser::{ClassToken, ResponsivePrefix, StatePrefix};

/// Emit CSS for a list of class tokens. Output is deterministic so repeated
/// builds produce identical CSS (good for caching + diffing).
pub fn emit_css(tokens: &[ClassToken]) -> String {
    // Group tokens by (responsive, state) so we can emit @media + pseudo
    // selectors efficiently.
    let mut groups: BTreeMap<
        Option<ResponsivePrefix>,
        HashMap<Option<StatePrefix>, Vec<&ClassToken>>,
    > = BTreeMap::new();

    for t in tokens {
        groups
            .entry(t.responsive.clone())
            .or_default()
            .entry(t.state.clone())
            .or_default()
            .push(t);
    }

    let mut out = String::with_capacity(2048);
    out.push_str("/* kungfu-css bundle — auto-generated */\n");

    for (responsive, state_groups) in &groups {
        if let Some(r) = responsive {
            out.push_str(&format!("@media (min-width: {}px) {{\n", r.min_width_px()));
        }

        for (state, tokens) in state_groups {
            for t in tokens {
                let declarations = match utility_to_css(&t.utility) {
                    Some(d) => d,
                    None => continue, // unknown utility — skip
                };
                let escaped = t.selector();
                let pseudo = state.as_ref().map(|s| s.css_pseudo()).unwrap_or("");
                out.push_str(&format!(".{}{} {{\n", escaped, pseudo));
                for decl in declarations {
                    out.push_str(&format!("  {};\n", decl));
                }
                out.push_str("}\n");
            }
        }

        if responsive.is_some() {
            out.push_str("}\n");
        }
    }

    out
}

/// Map a utility name (e.g. `p-4`, `text-red-500`) to a list of CSS
/// declarations. Returns `None` for unrecognised utilities.
fn utility_to_css(name: &str) -> Option<Vec<String>> {
    // Layout.
    match name {
        "block" => return Some(vec!["display: block".into()]),
        "inline" => return Some(vec!["display: inline".into()]),
        "inline-block" => return Some(vec!["display: inline-block".into()]),
        "flex" => return Some(vec!["display: flex".into()]),
        "inline-flex" => return Some(vec!["display: inline-flex".into()]),
        "grid" => return Some(vec!["display: grid".into()]),
        "hidden" => return Some(vec!["display: none".into()]),
        "relative" => return Some(vec!["position: relative".into()]),
        "absolute" => return Some(vec!["position: absolute".into()]),
        "fixed" => return Some(vec!["position: fixed".into()]),
        "static" => return Some(vec!["position: static".into()]),
        "sticky" => return Some(vec!["position: sticky".into()]),
        "flex-row" => return Some(vec!["flex-direction: row".into()]),
        "flex-col" => return Some(vec!["flex-direction: column".into()]),
        "flex-wrap" => return Some(vec!["flex-wrap: wrap".into()]),
        "flex-nowrap" => return Some(vec!["flex-wrap: nowrap".into()]),
        "flex-1" => return Some(vec!["flex: 1 1 0%".into()]),
        "flex-auto" => return Some(vec!["flex: 1 1 auto".into()]),
        "flex-none" => return Some(vec!["flex: none".into()]),
        "items-center" => return Some(vec!["align-items: center".into()]),
        "items-start" => return Some(vec!["align-items: flex-start".into()]),
        "items-end" => return Some(vec!["align-items: flex-end".into()]),
        "items-stretch" => return Some(vec!["align-items: stretch".into()]),
        "items-baseline" => return Some(vec!["align-items: baseline".into()]),
        "justify-center" => return Some(vec!["justify-content: center".into()]),
        "justify-between" => return Some(vec!["justify-content: space-between".into()]),
        "justify-start" => return Some(vec!["justify-content: flex-start".into()]),
        "justify-end" => return Some(vec!["justify-content: flex-end".into()]),
        "justify-around" => return Some(vec!["justify-content: space-around".into()]),
        "justify-evenly" => return Some(vec!["justify-content: space-evenly".into()]),
        "self-auto" => return Some(vec!["align-self: auto".into()]),
        "self-start" => return Some(vec!["align-self: flex-start".into()]),
        "self-end" => return Some(vec!["align-self: flex-end".into()]),
        "self-center" => return Some(vec!["align-self: center".into()]),
        "gap-1" => return Some(vec!["gap: 0.25rem".into()]),
        "gap-2" => return Some(vec!["gap: 0.5rem".into()]),
        "gap-4" => return Some(vec!["gap: 1rem".into()]),
        "gap-6" => return Some(vec!["gap: 1.5rem".into()]),
        "gap-8" => return Some(vec!["gap: 2rem".into()]),
        _ => {}
    }

    // Typography.
    match name {
        "italic" => return Some(vec!["font-style: italic".into()]),
        "font-bold" => return Some(vec!["font-weight: 700".into()]),
        "font-semibold" => return Some(vec!["font-weight: 600".into()]),
        "font-medium" => return Some(vec!["font-weight: 500".into()]),
        "font-normal" => return Some(vec!["font-weight: 400".into()]),
        "font-light" => return Some(vec!["font-weight: 300".into()]),
        "text-center" => return Some(vec!["text-align: center".into()]),
        "text-left" => return Some(vec!["text-align: left".into()]),
        "text-right" => return Some(vec!["text-align: right".into()]),
        _ => {}
    }

    // Text size.
    let text_size = match name {
        "text-xs" => Some("0.75rem"),
        "text-sm" => Some("0.875rem"),
        "text-base" => Some("1rem"),
        "text-lg" => Some("1.125rem"),
        "text-xl" => Some("1.25rem"),
        "text-2xl" => Some("1.5rem"),
        "text-3xl" => Some("1.875rem"),
        "text-4xl" => Some("2.25rem"),
        _ => None,
    };
    if let Some(size) = text_size {
        return Some(vec![format!("font-size: {}", size)]);
    }

    // Borders / radius.
    match name {
        "border" => return Some(vec!["border-width: 1px".into()]),
        "border-0" => return Some(vec!["border-width: 0".into()]),
        "border-2" => return Some(vec!["border-width: 2px".into()]),
        "rounded" => return Some(vec!["border-radius: 0.25rem".into()]),
        "rounded-sm" => return Some(vec!["border-radius: 0.125rem".into()]),
        "rounded-md" => return Some(vec!["border-radius: 0.375rem".into()]),
        "rounded-lg" => return Some(vec!["border-radius: 0.5rem".into()]),
        "rounded-xl" => return Some(vec!["border-radius: 0.75rem".into()]),
        "rounded-2xl" => return Some(vec!["border-radius: 1rem".into()]),
        "rounded-full" => return Some(vec!["border-radius: 9999px".into()]),
        _ => {}
    }

    // Pattern-matched utilities (prefix-N).
    if let Some(rest) = name.strip_prefix("p-") {
        if let Some(n) = parse_spacing(rest) {
            return Some(vec![format!("padding: {}", spacing_value(n))]);
        }
    }
    if let Some(rest) = name.strip_prefix("px-") {
        if let Some(n) = parse_spacing(rest) {
            return Some(vec![format!("padding-left: {}", spacing_value(n)), format!("padding-right: {}", spacing_value(n))]);
        }
    }
    if let Some(rest) = name.strip_prefix("py-") {
        if let Some(n) = parse_spacing(rest) {
            return Some(vec![format!("padding-top: {}", spacing_value(n)), format!("padding-bottom: {}", spacing_value(n))]);
        }
    }
    if let Some(rest) = name.strip_prefix("m-") {
        if let Some(n) = parse_spacing(rest) {
            return Some(vec![format!("margin: {}", spacing_value(n))]);
        }
    }
    if let Some(rest) = name.strip_prefix("mx-") {
        if let Some(n) = parse_spacing(rest) {
            return Some(vec![format!("margin-left: {}", spacing_value(n)), format!("margin-right: {}", spacing_value(n))]);
        }
    }
    if let Some(rest) = name.strip_prefix("my-") {
        if let Some(n) = parse_spacing(rest) {
            return Some(vec![format!("margin-top: {}", spacing_value(n)), format!("margin-bottom: {}", spacing_value(n))]);
        }
    }
    if let Some(rest) = name.strip_prefix("w-") {
        if rest == "full" { return Some(vec!["width: 100%".into()]); }
        if let Some(n) = rest.parse::<u32>().ok() {
            return Some(vec![format!("width: {}px", n)]);
        }
    }
    if let Some(rest) = name.strip_prefix("h-") {
        if rest == "full" { return Some(vec!["height: 100%".into()]); }
        if let Some(n) = rest.parse::<u32>().ok() {
            return Some(vec![format!("height: {}px", n)]);
        }
    }

    // Colors.
    if let Some(rest) = name.strip_prefix("text-") {
        if let Some(color) = parse_color(rest) {
            return Some(vec![format!("color: {}", color)]);
        }
    }
    if let Some(rest) = name.strip_prefix("bg-") {
        if let Some(color) = parse_color(rest) {
            return Some(vec![format!("background-color: {}", color)]);
        }
    }
    if let Some(rest) = name.strip_prefix("border-") {
        if let Some(color) = parse_color(rest) {
            return Some(vec![format!("border-color: {}", color)]);
        }
    }

    None
}

fn parse_spacing(s: &str) -> Option<u32> {
    s.parse::<u32>().ok().filter(|&n| n <= 16)
}

fn spacing_value(n: u32) -> String {
    // Tailwind's spacing scale: 1 unit = 0.25rem (4px).
    format!("{}rem", n as f32 * 0.25)
}

/// Map a color name like `red-500` to a CSS color value.
fn parse_color(input: &str) -> Option<String> {
    let colors: &[(&str, &[(&str, &str)])] = &[
        ("red", &[
            ("100", "#fee2e2"), ("200", "#fecaca"), ("300", "#fca5a5"),
            ("400", "#f87171"), ("500", "#ef4444"), ("600", "#dc2626"),
            ("700", "#b91c1c"), ("800", "#991b1b"), ("900", "#7f1d1d"),
        ]),
        ("blue", &[
            ("100", "#dbeafe"), ("200", "#bfdbfe"), ("300", "#93c5fd"),
            ("400", "#60a5fa"), ("500", "#3b82f6"), ("600", "#2563eb"),
            ("700", "#1d4ed8"), ("800", "#1e40af"), ("900", "#1e3a8a"),
        ]),
        ("green", &[
            ("100", "#dcfce7"), ("200", "#bbf7d0"), ("300", "#86efac"),
            ("400", "#4ade80"), ("500", "#22c55e"), ("600", "#16a34a"),
            ("700", "#15803d"), ("800", "#166534"), ("900", "#14532d"),
        ]),
        ("gray", &[
            ("100", "#f3f4f6"), ("200", "#e5e7eb"), ("300", "#d1d5db"),
            ("400", "#9ca3af"), ("500", "#6b7280"), ("600", "#4b5563"),
            ("700", "#374151"), ("800", "#1f2937"), ("900", "#111827"),
        ]),
        ("yellow", &[
            ("100", "#fef3c7"), ("200", "#fde68a"), ("300", "#fcd34d"),
            ("400", "#fbbf24"), ("500", "#f59e0b"), ("600", "#d97706"),
            ("700", "#b45309"), ("800", "#92400e"), ("900", "#78350f"),
        ]),
        ("purple", &[
            ("100", "#f3e8ff"), ("200", "#e9d5ff"), ("300", "#d8b4fe"),
            ("400", "#c084fc"), ("500", "#a855f7"), ("600", "#9333ea"),
            ("700", "#7e22ce"), ("800", "#6b21a8"), ("900", "#581c87"),
        ]),
    ];

    for (color_name, shades) in colors {
        if let Some(rest) = input.strip_prefix(&format!("{}-", color_name)) {
            for (shade, hex) in *shades {
                if rest == *shade {
                    return Some(hex.to_string());
                }
            }
        }
    }

    // Also accept bare colour names: text-red, bg-blue, etc. → use the 500 shade.
    for (color_name, shades) in colors {
        if input == *color_name {
            for (shade, hex) in *shades {
                if *shade == "500" {
                    return Some(hex.to_string());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_class_string;

    #[test]
    fn emits_basic_layout() {
        let tokens = parse_class_string("flex items-center justify-center");
        let css = emit_css(&tokens);
        assert!(css.contains(".flex"));
        assert!(css.contains("display: flex"));
        assert!(css.contains(".items-center"));
        assert!(css.contains("align-items: center"));
    }

    #[test]
    fn emits_spacing() {
        let tokens = parse_class_string("p-4 px-2 m-8");
        let css = emit_css(&tokens);
        assert!(css.contains(".p-4"));
        assert!(css.contains("padding: 1rem"));
        assert!(css.contains(".px-2"));
        assert!(css.contains("padding-left: 0.5rem"));
        assert!(css.contains("padding-right: 0.5rem"));
        assert!(css.contains(".m-8"));
        assert!(css.contains("margin: 2rem"));
    }

    #[test]
    fn emits_colors() {
        let tokens = parse_class_string("text-red-500 bg-blue-200");
        let css = emit_css(&tokens);
        assert!(css.contains(".text-red-500"));
        assert!(css.contains("color: #ef4444"));
        assert!(css.contains(".bg-blue-200"));
        assert!(css.contains("background-color: #bfdbfe"));
    }

    #[test]
    fn groups_responsive_under_media_query() {
        let tokens = parse_class_string("flex md:flex lg:flex");
        let css = emit_css(&tokens);
        assert!(css.contains("@media (min-width: 768px)"));
        assert!(css.contains("@media (min-width: 1024px)"));
    }

    #[test]
    fn emits_flexbox_expansion() {
        let tokens = parse_class_string("flex-1 flex-col gap-4 justify-around self-center");
        let css = emit_css(&tokens);
        assert!(css.contains(".flex-1"));
        assert!(css.contains("flex: 1 1 0%"));
        assert!(css.contains(".flex-col"));
        assert!(css.contains("flex-direction: column"));
        assert!(css.contains(".gap-4"));
        assert!(css.contains("gap: 1rem"));
        assert!(css.contains(".justify-around"));
        assert!(css.contains("justify-content: space-around"));
        assert!(css.contains(".self-center"));
        assert!(css.contains("align-self: center"));
    }
}
