use txtview::{TxtView, TxtViewConfig};

fn main() -> std::io::Result<()> {
    let text = (1..=100)
        .map(|i| format!("Line {:>3}: The quick brown fox jumps over the lazy dog", i))
        .collect::<Vec<String>>()
        .join("\n");

    let config = TxtViewConfig {
        show_line_numbers: true,
        ..TxtViewConfig::default()
    };

    let mut viewer = TxtView::new(&text).with_config(config);
    viewer.run()
}
