# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Every control character in wrapped rows measures as exactly two columns, so the wrapping width can no longer be misread as leaving one column for some controls.
- The scrollbar geometry guard no longer checks for an impossible zero visible area, the presence of the bar is decided by the scrollbar-reserved flag alone.
- The input text is stored once in a single buffer with a byte-offset line index instead of one `String` per line, matching `str::lines()` semantics (CRLF, blank lines and a final trailing newline included) while cutting the per-line memory overhead and the allocation count from thousands to a handful.
- Expanded the integration suite from four ad-hoc construction checks into three focused binaries (`tests/{config,construction,contract}.rs`) that pin the public API the way downstream crates use it: `str::lines()`-matching line counts including CRLF and multibyte input, every `with_*` builder and config round-trip and the `Send + Sync`, `Clone` and `Debug` guarantees.
- The `NotConnected` error from `run()` on a non-terminal is tested end-to-end by re-running the test binary with its stdout piped, including a check that the child actually executed the assertion.

## [0.1.3] - 2026-09-23

### Added

- Colon-form SGR colors fold into the compact attribute state, including the 58/59 underline-color codes and `:`-style modifiers fold instead of leaking as raw attributes.
- `view_file` example opens UTF-16 and UTF-32 files and skips oversized ones.
- CI: tests on Ubuntu, macOS and Windows, plus fmt and clippy, with an MSRV check on Rust 1.85.
- Declared Rust 1.85 as the MSRV (edition 2024) and restored 1.85 compatibility.
- README: CI status badge, MSRV requirement, `narrow_viewports` example and a security-reporting section.
- [`SECURITY.md`](SECURITY.md) vulnerability reporting policy.
- GitHub issue templates, with `.github/` excluded from the published crate.

### Changed

- Rendering rebuilt as a canvas of components (content, help bar, scrollbar). Every component draws through one write path.
- Emoji-presentation clusters measure two columns.
- Tab gaps paint as spaces so scrolling leaves no stale cells.
- Help text wraps with the content rows. The help bar hides instead of flooding narrow viewports.
- The scrollbar reserves its column only when content overflows and the text keeps at least three columns. The thumb rounds to the nearest cell and its reach is capped.
- Grabbing the thumb aims at the same center a track press uses. A track press that arms the grab without moving redraws.
- Immediate `PgUp`/`PgDn` at startup is accepted.
- Letter-key shortcuts are skipped when any modifier key is held.
- `TxtView`'s `Debug` prints counts instead of the whole document.
- Added clippy and rustfmt configuration.

### Fixed

- Preserved SGR replay order, dropped malformed colors and guaranteed a wrapped row never overflows its buffer.
- The final flush error at teardown is reported instead of being swallowed.
- Hardened viewport sizing and removed the `u16::MAX` fallback footgun. Fixed viewport sizes are documented to clamp to the terminal.

## [0.1.2] - 2026-09-12

### Added

- `Debug` and `Clone` derives and a `config()` getter on `TxtView`.
- Wrapping by extended grapheme cluster so skintone modifiers, ZWJ families, flag pairs and combining marks never split across a row boundary.
- Piped stdin support by not rejecting non-terminal stdin.
- A live SGR style state machine replacing flat attribute accumulation.
- `visual_width` example refreshed for column-aware wrapping.

### Changed

- The terminal is restored from a `Drop` guard so a panic cannot strand raw mode.
- Shorter page-jump cooldown. The last-page jump is split into page-up and page-down.
- Control bytes are neutralized and non-SGR escapes render as visible text. Control characters are neutralized in wrapping.
- The help bar caps to available rows and always reserves at least one content row.
- The document re-wraps only when layout geometry changes, not on every scroll.
- The viewport height clamps to the terminal and anchors the help bar to it.
- Tabs measure by their 8-column terminal stops.

### Fixed

- Panic on CSI escapes terminated by a multi-byte character.

## [0.1.1] - 2026-09-10

### Added

- Output coverage for `rebuild_display`, plus CJK, Japanese and Korean sample lines in the `visual_width` example.
- `#[must_use]` on builder methods. `term_size()` and `help_text()` are associated functions.

### Changed

- Replaced truncating/wrapping numeric casts with checked conversions.
- Consistent terminal size fallback in `draw()`.
- Removed the redundant display rebuild on every scroll event and dead `rows_per_line` code.
- Extracted viewport dimension resolution helpers.

### Fixed

- Multi-line scroll skip on Windows by filtering key repeat events.
- Layout rendering for ANSI, wide characters and scrollbar overlap.

## [0.1.0] - 2026-09-09

### Added

- Initial release
