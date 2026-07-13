//! ColorScheme — 5 preset color themes for the FerroCopy GUI.
//!
//! Inspired by WezTerm's color scheme system. Each preset maps to
//! egui::Style colors for a consistent visual theme.

use egui::Color32;

/// A named color scheme with egui-compatible colors.
#[derive(Debug, Clone)]
pub struct ColorScheme {
    pub name: &'static str,
    pub bg: Color32,
    pub fg: Color32,
    pub accent: Color32,
    pub success: Color32,
    pub error: Color32,
    pub warn: Color32,
    pub progress_fill: Color32,
    pub progress_bg: Color32,
}

/// All available color schemes.
pub const SCHEMES: &[ColorScheme] = &[
    ColorScheme {
        name: "Deep Space",
        bg: Color32::from_rgb(10, 10, 20),
        fg: Color32::from_rgb(200, 200, 210),
        accent: Color32::from_rgb(100, 180, 255),
        success: Color32::from_rgb(76, 175, 80),
        error: Color32::from_rgb(244, 67, 54),
        warn: Color32::from_rgb(255, 200, 50),
        progress_fill: Color32::from_rgb(80, 180, 255),
        progress_bg: Color32::from_rgb(30, 30, 50),
    },
    ColorScheme {
        name: "Midnight Nebula",
        bg: Color32::from_rgb(5, 5, 25),
        fg: Color32::from_rgb(180, 180, 220),
        accent: Color32::from_rgb(120, 80, 255),
        success: Color32::from_rgb(0, 200, 150),
        error: Color32::from_rgb(255, 80, 80),
        warn: Color32::from_rgb(255, 200, 50),
        progress_fill: Color32::from_rgb(120, 80, 255),
        progress_bg: Color32::from_rgb(20, 20, 40),
    },
    ColorScheme {
        name: "Solar Flare",
        bg: Color32::from_rgb(20, 15, 10),
        fg: Color32::from_rgb(220, 200, 170),
        accent: Color32::from_rgb(255, 150, 50),
        success: Color32::from_rgb(100, 200, 100),
        error: Color32::from_rgb(255, 80, 50),
        warn: Color32::from_rgb(255, 200, 50),
        progress_fill: Color32::from_rgb(255, 150, 50),
        progress_bg: Color32::from_rgb(40, 30, 20),
    },
    ColorScheme {
        name: "Aurora",
        bg: Color32::from_rgb(10, 20, 15),
        fg: Color32::from_rgb(180, 220, 200),
        accent: Color32::from_rgb(50, 255, 150),
        success: Color32::from_rgb(50, 200, 100),
        error: Color32::from_rgb(255, 80, 80),
        warn: Color32::from_rgb(255, 220, 50),
        progress_fill: Color32::from_rgb(50, 255, 150),
        progress_bg: Color32::from_rgb(15, 30, 20),
    },
    ColorScheme {
        name: "Blood Moon",
        bg: Color32::from_rgb(15, 5, 5),
        fg: Color32::from_rgb(200, 150, 150),
        accent: Color32::from_rgb(255, 50, 50),
        success: Color32::from_rgb(200, 100, 100),
        error: Color32::from_rgb(255, 0, 0),
        warn: Color32::from_rgb(255, 150, 50),
        progress_fill: Color32::from_rgb(200, 50, 50),
        progress_bg: Color32::from_rgb(30, 10, 10),
    },
];

/// Apply a color scheme to an egui Style.
pub fn apply(scheme: &ColorScheme, style: &mut egui::Style) {
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = scheme.bg;
    style.visuals.window_fill = scheme.bg;
    style.visuals.faint_bg_color = scheme.bg;
    style.visuals.extreme_bg_color = scheme.bg;
    style.visuals.code_bg_color = scheme.bg;
    style.visuals.widgets.noninteractive.bg_fill = scheme.bg;
    style.visuals.widgets.noninteractive.fg_stroke.color = scheme.fg;
    style.visuals.widgets.inactive.fg_stroke.color = scheme.fg;
    style.visuals.widgets.active.fg_stroke.color = scheme.accent;
    style.visuals.hyperlink_color = scheme.accent;
    style.visuals.selection.stroke.color = scheme.accent;
}

/// Get a scheme by index (0-based).
pub fn get_scheme(index: usize) -> Option<&'static ColorScheme> {
    SCHEMES.get(index)
}

/// Find the index of a scheme by name.
pub fn scheme_index(name: &str) -> Option<usize> {
    SCHEMES.iter().position(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheme_count() {
        assert_eq!(SCHEMES.len(), 5);
    }

    #[test]
    fn test_get_scheme() {
        let scheme = get_scheme(0).unwrap();
        assert_eq!(scheme.name, "Deep Space");
    }

    #[test]
    fn test_scheme_index() {
        assert_eq!(scheme_index("Aurora"), Some(3));
        assert_eq!(scheme_index("Unknown"), None);
    }

    #[test]
    fn test_apply_does_not_panic() {
        let scheme = &SCHEMES[0];
        let mut style = egui::Style::default();
        apply(scheme, &mut style);
        assert!(style.visuals.dark_mode);
    }

    #[test]
    fn test_all_schemes_have_unique_names() {
        let mut names: Vec<&str> = SCHEMES.iter().map(|s| s.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), SCHEMES.len());
    }
}