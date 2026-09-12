# TxtView

[![crates.io](https://img.shields.io/crates/v/txtview.svg)](https://crates.io/crates/txtview)
[![docs.rs](https://img.shields.io/docsrs/txtview)](https://docs.rs/txtview)

**TxtView** is a lightweight text viewer for the terminal, written in Rust and built on [crossterm](https://github.com/crossterm-rs/crossterm). Drop it into your own CLI tools whenever you need a line-based viewer with scrolling, wrapping, line numbers, and an optional interactive scrollbar.

## Features

- View any text in an alternate screen terminal session
- Scroll line-by-line `↑/↓` or `j/k`, by page `PgUp`/`PgDn`, or with the mouse wheel
- Jump to the start `Home`/`g` or the end `End`/`G` of the text
- Automatic line wrapping to the viewport width, with continuation markers on wrapped rows (in the line-number column)
- Optional line numbers
- Optional help bar with keybinding hints
- Optional interactive vertical scrollbar
- Configurable viewport width and height, or just use the terminal size automatically

## Library Usage

Add `txtview` to your `Cargo.toml`:

```toml
[dependencies]
txtview = "0.1"
```

Quick start:

```rust
use txtview::{TxtView, TxtViewConfig};

fn main() {
    let text = "hello world";

    let config = TxtViewConfig {
        show_line_numbers: true,
        ..TxtViewConfig::default()
    };

    let mut viewer = TxtView::new(&text).with_config(config);
    viewer.run().unwrap();
}
```

## Styled Text

TxtView renders ANSI-styled text correctly: SGR color and style codes, plus OSC8 hyperlinks, pass through untouched, and when a styled line wraps the active style is preserved and re-emitted as a compact prefix on the wrapped rows. Control bytes and any other escape sequence are shown as visible caret notation (`^G`, `^[[2A`) rather than executed. Build your styled strings with [crossterm](https://github.com/crossterm-rs/crossterm) (the same library TxtView uses internally, so adding it costs no extra build time):

```toml
[dependencies]
txtview = "0.1"
crossterm = "0.29"
```

```rust
use crossterm::style::Stylize;
use txtview::{TxtView, TxtViewConfig};

fn main() {
    let text = format!(
        "Status: {}\n  · {}\n  · {}",
        "running".green().bold(),
        "in a sandbox".yellow().italic(),
        "high disk usage".red()
    );

    let mut viewer = TxtView::new(&text).with_config(TxtViewConfig::default());
    viewer.run().unwrap();
}
```

You are not tied to crossterm for styling. Any library that emits SGR styling codes, or even raw sequences written by hand, works just as well (other escape sequences are neutralized to visible text, so only styling and OSC8 links register):

```rust
let text = "\x1b[1;32mrunning\x1b[0m";
```

## Configuration

TxtView is configured through `TxtViewConfig`, either with `..TxtViewConfig::default()` to fill the remaining fields, or with chained `with_*` setters:

```rust
let config = TxtViewConfig::default()
    .with_show_line_numbers(true)
    .with_show_scrollbar(false);
```

It can toggle line numbers, the help bar, and the scrollbar, as well as fix the viewport width and height (it falls back to the terminal size when unset).

For the full list of options and defaults, see the `TxtViewConfig` rustdoc.

## Keybindings

| Key              | Action                    |
| ---------------- | ------------------------- |
| `q` / `Esc` / `Ctrl+C` | Quit                |
| `↑` / `↓`, `j` / `k`      | Scroll one line      |
| `PgUp` / `PgDn`  | Scroll one page      |
| `Home` / `g`     | Jump to start        |
| `End` / `G`      | Jump to end          |
| Mouse wheel      | Scroll one line per tick |
| Scrollbar track / thumb | Click to jump to a position, drag to scroll |

## Examples

```bash
# Show a generated sample document
cargo run --example sample_text

# View an arbitrary file
cargo run --example view_file -- path/to/file.txt

# Show the style gallery
cargo run --example styled

# Show control bytes and non-SGR escapes as visible caret notation
cargo run --example control_chars

# Show column-aware wrapping with CJK, emoji, and full-width punctuation
cargo run --example visual_width

# Show how grapheme clusters (skintone modifiers, ZWJ families, flags) never split across a wrap
cargo run --example grapheme_clusters
```

## Building from Source

```bash
git clone https://github.com/iamdevstand/txtview
cd txtview
cargo build --release
```

## Tests

```bash
cargo test
```

## License

MIT License - see [LICENSE](LICENSE) for full details.

***

*TxtView © 2026 Devstand.*
