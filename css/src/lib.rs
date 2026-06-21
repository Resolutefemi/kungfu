//! kungfu-css — a Tailwind-like utility CSS engine for the Kungfu.js framework.
//!
//! Parses utility class names (`flex`, `p-4`, `text-red-500`, `md:hover:bg-blue-200`)
//! and emits CSS rules. Scans source files to extract the set of used classes
//! and produces a single minimal CSS file.
//!
//! ## Why a custom engine?
//!
//! Tailwind CSS is excellent, but it ships 30+ MB of CSS in development mode
//! and its JIT compiler is a Node.js dependency. We want Kungfu's frontend
//! story to be dependency-light and fast — the CSS engine is part of the
//! Rust core, so `kungfu build` produces a CSS bundle in microseconds without
//! spawning a Node process.
//!
//! ## Supported utilities (V1)
//!
//! - Layout: `block`, `inline`, `flex`, `grid`, `hidden`, `relative`, `absolute`
//! - Spacing: `p-{n}`, `px-{n}`, `py-{n}`, `m-{n}`, `mx-{n}`, `my-{n}` (0–16)
//! - Colors: `text-{color}-{shade}`, `bg-{color}-{shade}` (red, blue, green, gray, 100–900)
//! - Typography: `text-{xs|sm|base|lg|xl|2xl|3xl}`, `font-{bold|medium|normal}`, `italic`
//! - Borders: `border`, `border-{n}`, `rounded`, `rounded-{lg|md|sm|full}`
//! - Display: `w-{n}`, `h-{n}`, `w-full`, `h-full`
//! - Responsive: `sm:`, `md:`, `lg:`, `xl:` prefixes
//! - State: `hover:`, `focus:` prefixes
//!
//! ## Example
//!
//! ```ignore
//! use kungfu_css::{compile_directory, compile_classes};
//!
//! // Compile a single class string:
//! let css = compile_classes("flex p-4 text-red-500");
//!
//! // Scan a directory and produce a CSS bundle:
//! let css = compile_directory("./src").unwrap();
//! ```

pub mod parser;
pub mod emitter;
pub mod scanner;

use std::path::Path;

pub use emitter::emit_css;
pub use parser::{parse_class, parse_class_string, ClassToken, ResponsivePrefix, StatePrefix};
pub use scanner::{scan_directory, scan_file};

/// Top-level convenience: compile a single class string to CSS.
pub fn compile_classes(input: &str) -> String {
    let tokens = parse_class_string(input);
    emit_css(&tokens)
}

/// Top-level convenience: scan a directory recursively for `.html`, `.kungfu`,
/// `.tsx`, `.jsx` files, extract used classes, and emit a CSS bundle.
pub fn compile_directory<P: AsRef<Path>>(root: P) -> std::io::Result<String> {
    let classes = scan_directory(root)?;
    let tokens: Vec<ClassToken> = classes
        .iter()
        .flat_map(|c| parse_class_string(c))
        .collect();
    Ok(emit_css(&tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_basic_utilities() {
        let css = compile_classes("flex p-4 text-red-500");
        assert!(css.contains(".flex"));
        assert!(css.contains(".p-4"));
        assert!(css.contains(".text-red-500"));
    }

    #[test]
    fn handles_responsive_prefix() {
        let css = compile_classes("md:flex");
        assert!(css.contains("@media (min-width: 768px)"));
        assert!(css.contains(".md\\:flex"));
    }

    #[test]
    fn handles_hover_state() {
        let css = compile_classes("hover:bg-blue-500");
        assert!(css.contains(".hover\\:bg-blue-500:hover"));
    }

    #[test]
    fn ignores_unknown_classes() {
        let css = compile_classes("my-custom-class p-4");
        // Unknown classes are silently skipped (no CSS emitted for them).
        assert!(!css.contains("my-custom-class"));
        assert!(css.contains(".p-4"));
    }
}
