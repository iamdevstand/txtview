use txtview::{TxtView, TxtViewConfig};

fn main() -> std::io::Result<()> {
    // Fixed 20-column viewport so wrapping always happens at the same place,
    // independent of the terminal size.
    let doc = vec![
        "Grapheme cluster wrapping demo".to_string(),
        "".to_string(),
        "Every cluster wraps as one unit, never split at a row boundary:".to_string(),
        "".to_string(),
        "skin-tone modifier: 123456789012345678👍🏿xyz".to_string(),
        "ZWJ family emoji:   123456789012345678👨\u{200d}👩\u{200d}👧x".to_string(),
        "flag pair:          1234567890123456789🇺🇸x".to_string(),
        "".to_string(),
        "combining marks stay with their base char:".to_string(),
        "  na\u{303}i\u{303}ve cafe\u{301} re\u{301}sume\u{301}".to_string(),
        "".to_string(),
        "keycap and variation-selector sequences:".to_string(),
        "  \u{23}\u{fe0f}\u{20e3} \u{31}\u{fe0f}\u{20e3} vs plain 3 and a ❤\u{fe0f} heart"
            .to_string(),
        "".to_string(),
        "All of these stay intact even when a row boundary".to_string(),
        "lands in the middle of one.".to_string(),
    ];

    let text = doc.join("\n");

    let config = TxtViewConfig {
        viewport_width: Some(20),
        show_line_numbers: false,
        show_scrollbar: false,
        ..TxtViewConfig::default()
    };

    let mut viewer = TxtView::new(&text).with_config(config);
    viewer.run()
}
