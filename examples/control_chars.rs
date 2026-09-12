use txtview::{TxtView, TxtViewConfig};

fn main() -> std::io::Result<()> {
    let doc = vec![
        "Control character & tab handling demo".to_string(),
        "".to_string(),
        "Tabs are measured and wrapped at their 8-column terminal stop:".to_string(),
        "\tone tab of indentation".to_string(),
        "\t\ttwo tabs".to_string(),
        "\t\t\tthree tabs".to_string(),
        "no\tgap\tor\twider".to_string(),
        "".to_string(),
        "Mixed tab and space indentation:".to_string(),
        "    four spaces then\tone tab".to_string(),
        "\tone tab then    four spaces".to_string(),
        "".to_string(),
        "Control bytes are shown as caret notation instead of being executed:".to_string(),
        "backspace \x08 BEL \x07 CR \r DEL \x7f end".to_string(),
        "solo control bytes: ^not caret, actual: \x01 \x02 \x03 \x04".to_string(),
        "field separators: US \x1f RS \x1e GS \x1d FS \x1c".to_string(),
        "".to_string(),
        "SGR and OSC8 hyperlinks pass through; other escapes show as text:".to_string(),
        "cursor moves and clears never execute: \x1b[2A \x1b[2J \x1b[K".to_string(),
        "OSC8 hyperlink stays live and is never split: \x1b]8;;https://example.com/about\x07example.com\x1b]8;;\x07".to_string(),
        "an OSC title is escaped text, never a live title: \x1b]0;window title\x07".to_string(),
        "two-byte escapes too: \x1b7 saved cursor \x1b8 restore".to_string(),
        "".to_string(),
        "An empty line and a tab-only line both occupy a row:".to_string(),
        "".to_string(),
        "\t".to_string(),
        "".to_string(),
        "Long content so wrapping kicks in, note how tabs keep 8-column alignment:".to_string(),
        "\tLorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.".to_string(),
        "a shorter line\t\t\twith long tab gaps       and trailing spaces".to_string(),
        "".to_string(),
        "A very long unbroken\t\t\ttoken-heavy line that wraps several times across the viewport width to exercise the wrap-with-tab path:".to_string(),
        "".to_string(),
    ];

    let text = doc.join("\n");

    let config = TxtViewConfig {
        show_line_numbers: true,
        show_scrollbar: true,
        show_help_bar: true,
        ..TxtViewConfig::default()
    };

    let mut viewer = TxtView::new(&text).with_config(config);
    viewer.run()
}
