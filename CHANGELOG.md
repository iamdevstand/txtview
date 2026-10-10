# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `From` implementations for `TxtView`: `TxtView::from` takes a `String` and moves its buffer into the viewer without a copy (a `Cow::Owned` buffer likewise, a `Cow::Borrowed` slice is copied). `new` continues to copy the input and now shares the construction path with it.
- `AsRef<str>` for `TxtView` (returns the document's text).
- `TxtView::lines()`, an iterator over the document's lines matching `str::lines()`, served from the byte-offset index built at construction (no buffer re-scan, no per-line allocation).
- `PartialEq` and `Eq` for `TxtViewConfig`: two configs compare equal when all fields match.
- The `view_file` example hands the decoded text to `TxtView::from`, so the example no longer copies the decoded text during construction (no transient 2× peak).

### Changed

- Every frame is drawn inside synchronized output (DEC 2026), so fast scrolling presents completed frames instead of half-painted ones. Fixes fast-scroll artifacts and scroll flicker.
- The wrap-count pass measures control bytes and escapes without building their display text, so counting a document's rows allocates nothing for them.
- A terminal `Resize` event lays out against its own reported size instead of re-querying it from the terminal.
- Style replay at a wrap boundary no longer allocates when nothing is styled: the SGR state answers `is_active` before anything is built and writes its codes straight into the new row instead of an intermediate parameter string, so wrapping a long line costs a small constant number of allocations per row instead of one extra allocation per boundary.
- The wrapped document is no longer materialized up front. A per-source-line row-count index (`row_layout`) replaces the full `Vec<String>` display and each visible row is wrapped on demand through an index lookup while a frame draws, so construction cost and memory track the document while per-frame wrap work tracks the visible window. Scrolling and mouse hit-testing wrap nothing at all.
- The content piece renders straight from a row provider that materializes one display row at a time, so a frame allocates a single row string instead of a windowful of pre-wrapped rows.
- The content piece folds each row's cursor move, text and padding into one reused byte buffer and commits them with a single write, so a frame costs one write per row instead of one `core::fmt` call per grapheme cluster.
- The scrollbar assembles a whole bar into one buffer and writes it in a single call, appending each cell's glyph bytes directly (the cursor escape still goes through `MoveTo`'s `Display`), so drawing the bar cuts `core::fmt` to one call per cell instead of two. The emitted escape stream is unchanged.
- Wrapping is driven by one resumable row-at-a-time scan (`WrappedLine`): the row-count index counts through it instead of a separate counter loop and a painted row skips every row before it without building them and materializes only the target one, so the painted row is wrapped once instead of re-wrapping the whole line, paying only to measure the rows it skips.
- Every control character in wrapped rows measures as exactly two columns, so the wrapping width can no longer be misread as leaving one column for some controls.
- `run()` reports a terminal restore that fails on the normal return path, where the `Drop` guard once dropped the teardown errors silently (the guard still cleans up on a panic, where reporting is impossible).
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
