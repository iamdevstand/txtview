---
name: Bug report
about: Something in TxtView isn't working as expected
title: "[bug] "
labels: bug
---

Thanks for reporting a bug! The more detail you provide, the faster it can be fixed. Feel free to remove any section that doesn't apply.

## What happened

<!-- A short, clear description of the problem. -->

## Steps to reproduce

<!-- Numbered steps, or the exact keys / mouse actions that trigger it. -->

1.
2.
3.

## Expected behavior

<!-- What you expected to happen. -->

## Actual behavior

<!-- What actually happened (including any error message or misrendering). -->

## Environment

- **OS** (e.g. Windows 11, macOS Sonoma, Ubuntu 24.04):
- **Terminal** (name + version, e.g. Windows Terminal 1.9, iTerm2 3.5):
- **Terminal size** (rows × columns, on macOS/Linux/WSL/Git Bash run `stty size`, which prints `ROWS COLUMNS`, e.g. `24 80`, on Windows run `mode con` in cmd or `(Get-Host).UI.RawUI.WindowSize` in PowerShell):
- **TxtView version** (the crate version or the commit hash if you use the git version):

## Offending content (optional)

<!-- If the bug depends on the actual text, include the smallest snippet that still reproduces it. If it has special characters (unicode, emoji, CJK, control bytes, ANSI escapes) that don't copy well, attach the file instead (a .txt or .md file) using the attach button below the form. -->

```text
```

## Code & config (optional)

<!-- If the bug depends on how TxtView was built, paste the relevant code here. -->

```rust
// `text` is the offending content from the section above, no need to repeat it here.
let config = TxtViewConfig {
    show_line_numbers: true,
    // show_scrollbar, help bar, fixed width/height, ...
    ..TxtViewConfig::default()
};
let mut viewer = TxtView::new(&text).with_config(config);
```
