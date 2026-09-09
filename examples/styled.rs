use crossterm::style::Stylize;
use txtview::{TxtView, TxtViewConfig};

fn bullet(name: &str, desc: &str, style: impl Fn(&str) -> String) -> String {
    format!("  • {} {}", style(&format!("{name:<22}")), desc)
}

fn binding(keys: &str, desc: &str) -> String {
    format!("  {} {}", format!("{keys:<11}").cyan(), desc)
}

fn setting(name: &str, desc: &str) -> String {
    format!("    {} {}", format!("{name:<22}").green().italic(), desc)
}

fn main() {
    let lines = vec![
        "Welcome to TxtView".bold().underlined().to_string(),
        "".to_string(),
        "What is this?".bold().to_string(),
        "  A lightweight terminal text viewer for Rust, built on".to_string(),
        format!(
            "  {} with no heavy dependencies.",
            "crossterm".cyan().bold()
        ),
        "".to_string(),
        "Supported features".bold().to_string(),
        bullet("Scrolling", "line-by-line or page-by-page", |s| {
            s.cyan().bold().to_string()
        }),
        bullet("Wrapping", "reflows text to the viewport width", |s| {
            s.yellow().to_string()
        }),
        bullet("Scrollbar", "shows the reading position", |s| {
            s.red().bold().to_string()
        }),
        bullet("Line numbers", "optional left-hand column", |s| {
            s.italic().to_string()
        }),
        bullet("Mouse support", "wheel and button scrolling", |s| {
            s.green().to_string()
        }),
        "".to_string(),
        "Keybindings".bold().to_string(),
        binding("j / k", "scroll one line"),
        binding("PgUp / PgDn", "scroll one page"),
        binding("g / G", "jump to start / end"),
        binding("q / Esc", "quit"),
        "".to_string(),
        "Configuration".bold().to_string(),
        "  Everything is set through TxtViewConfig:".to_string(),
        setting("show_line_numbers", "toggles the line number column"),
        setting("show_scrollbar", "toggles the interactive scrollbar"),
        setting("show_help_bar", "toggles this help section"),
        "".to_string(),
    ];

    let mut viewer = TxtView::new(lines.join("\n")).with_config(TxtViewConfig::default());
    viewer.run().unwrap();
}
