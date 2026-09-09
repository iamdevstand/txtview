# TxtView

**TxtView** is a lightweight text viewer for the terminal, written in Rust and built on [crossterm](https://github.com/crossterm-rs/crossterm). Drop it into your own CLI tools whenever you need a line-based viewer with scrolling, wrapping, line numbers, and an optional progress indicator.

## Features

- View any text in an alternate screen terminal session
- Scroll line-by-line `↑/↓` or `j/k`, by page `PgUp`/`PgDn`, or with the mouse wheel
- Jump to the start `Home`/`g` or the end `End`/`G` of the text
- Automatic line wrapping to the viewport width, with continuation markers on wrapped rows
- Optional line numbers
- Optional help bar with keybinding hints
- Optional vertical scrollbar progress indicator
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

TxtView writes your content verbatim, so any ANSI-styled text renders as-is. Build your styled strings with [crossterm](https://github.com/crossterm-rs/crossterm) (the same library TxtView uses internally, so adding it costs no extra build time):

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

You are not tied to crossterm for styling, any library that emits ANSI escapes, or even raw escape sequences written by hand, works just as well:

```rust
let text = "\x1b[1;32mrunning\x1b[0m";
```

## Configuration

TxtView is configured through `TxtViewConfig`, either with `..TxtViewConfig::default()` to fill the remaining fields, or with chained `with_*` setters:

```rust
let config = TxtViewConfig::default()
    .with_show_line_numbers(true)
    .with_show_progress(false);
```

It can toggle line numbers, the help bar, and the progress indicator, as well as fix the viewport width and height (it falls back to the terminal size when unset).

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

## Examples

```bash
# Show a generated sample document
cargo run --example sample_text

# View an arbitrary file
cargo run --example view_file -- path/to/file.txt

# Show the style gallery
cargo run --example styled
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
