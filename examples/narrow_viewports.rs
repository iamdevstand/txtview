use txtview::{TxtView, TxtViewConfig};

fn main() -> std::io::Result<()> {
    let text = [
        "A reference line that fits on one row so wrapping is visible:",
        "abc.",
        "",
        "A styled tab row wraps without overflowing the 6-column box:",
        "\x1b[31mab\tcd\x1b[0m",
        "",
        "Unknown SGR codes replay in first-seen order at every wrap:",
        "\x1b[62mx then \x1b[60my then plain\x1b[0m crawls across the box",
        "",
        "A malformed bare 38 is ignored, later colors still replay:",
        "\x1b[38m then \x1b[31mred stays red\x1b[0m",
        "",
        "Tabs on a viewport narrower than their stop become spaces:",
        "a\tb",
        "",
        "Wide emoji still wrap whole and keep double width:",
        "abcde🎉f",
        "",
    ]
    .join("\n");

    let config = TxtViewConfig {
        viewport_width: Some(6),
        viewport_height: Some(10),
        show_help_bar: false,
        show_scrollbar: false,
        ..TxtViewConfig::default()
    };

    let mut viewer = TxtView::new(text).with_config(config);
    viewer.run()
}
