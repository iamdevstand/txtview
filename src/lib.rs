#![warn(missing_docs)]
//! A minimal, crossterm-based terminal text viewer for Rust.
//!
//! Wrap any text with [`TxtView::new`], optionally tune it with
//! [`TxtViewConfig`], then call [`TxtView::run`]. The viewer takes over the
//! terminal with an alternate screen, switches to raw mode, and restores the
//! previous screen and settings when the user quits.
//!
//! [`TxtView::run`] blocks for the whole session and requires an interactive
//! terminal, so a small program is all it takes:
//!
//! ```no_run
//! # use txtview::TxtView;
//! # fn main() -> std::io::Result<()> {
//! let mut viewer = TxtView::new("hello world");
//! viewer.run()
//! # }
//! ```
//!
//! # Styled text
//!
//! The content is written verbatim, so ANSI colors and styling pass through
//! unchanged. Build styled lines with crossterm's `style` module and feed
//! them straight into [`TxtView::new`].
//!
//! # Configuration
//!
//! [`TxtViewConfig`] controls the display: line numbers, the help bar, the
//! progress scrollbar, and a fixed viewport size. Configure it with a struct
//! literal plus `..TxtViewConfig::default()`, or chain the `with_*` setters.
//! See its documentation for examples.

mod txtview;
mod txtview_config;

pub use txtview::TxtView;
pub use txtview_config::TxtViewConfig;
