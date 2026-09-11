use std::io;

use crossterm::{
    cursor::MoveTo,
    queue,
    terminal::{Clear, ClearType},
};

use super::TxtView;

impl TxtView {
    pub(super) fn draw(&mut self, stdout: &mut impl io::Write) -> io::Result<()> {
        let rows = self.resolved_height();
        self.refresh_bounds();

        let visible = usize::from(self.visible_rows());

        self.render_rows(stdout, 0..visible)?;
        self.render_scrollbar(stdout, visible)?;

        self.render_help_bar(stdout, rows)?;

        stdout.flush()?;
        Ok(())
    }

    fn render_rows(
        &mut self,
        stdout: &mut impl io::Write,
        range: std::ops::Range<usize>,
    ) -> io::Result<()> {
        for i in range {
            let text = self
                .display
                .get(self.offset + i)
                .map(String::as_str)
                .unwrap_or("");
            queue!(
                stdout,
                MoveTo(0, u16::try_from(i).unwrap_or(u16::MAX)),
                Clear(ClearType::CurrentLine)
            )?;
            write!(stdout, "{}", text)?;
        }
        Ok(())
    }

    fn render_scrollbar(&mut self, stdout: &mut impl io::Write, visible: usize) -> io::Result<()> {
        let Some(g) = self.scroll_geometry(visible) else {
            return Ok(());
        };
        for i in 0..g.visible {
            let ch = if i >= g.top && i < g.top + g.size {
                if self.dragging { '▓' } else { '█' }
            } else {
                '░'
            };
            queue!(
                stdout,
                MoveTo(g.column, u16::try_from(i).unwrap_or(u16::MAX))
            )?;
            write!(stdout, "{}", ch)?;
        }
        Ok(())
    }

    fn render_help_bar(&mut self, stdout: &mut impl io::Write, rows: u16) -> io::Result<()> {
        if !self.config.show_help_bar {
            return Ok(());
        }

        let total = usize::from(rows);
        if total < 2 {
            return Ok(());
        }

        let cols = self.help_wrap_cols();
        let lines = wrap_lines(&Self::help_text(), cols);
        let shown = lines.len().min(total - 2);
        let base = total - 1 - shown;

        queue!(
            stdout,
            MoveTo(0, u16::try_from(base).unwrap_or(u16::MAX)),
            Clear(ClearType::CurrentLine)
        )?;
        write!(stdout, "{}", "─".repeat(cols))?;

        for (i, line) in lines.iter().take(shown).enumerate() {
            queue!(
                stdout,
                MoveTo(0, u16::try_from(base + 1 + i).unwrap_or(u16::MAX)),
                Clear(ClearType::CurrentLine)
            )?;
            write!(stdout, "{}", line)?;
        }

        Ok(())
    }
}

fn wrap_lines(s: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars
        .chunks(width)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TxtViewConfig;

    fn viewer(n: usize) -> TxtView {
        let text = (0..n)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let config = TxtViewConfig {
            viewport_height: Some(10),
            viewport_width: Some(80),
            show_help_bar: false,
            ..TxtViewConfig::default()
        };
        TxtView::new(&text).with_config(config)
    }

    fn max_move_to_row(out: &[u8]) -> usize {
        let s = String::from_utf8_lossy(out);
        s.split("\x1b[")
            .filter_map(|m| {
                let rest = m.strip_suffix('H')?;
                rest.split_once(';')?.0.parse::<usize>().ok()
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn draw_clamps_oversized_viewport_to_terminal() {
        let text = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let config = TxtViewConfig {
            viewport_height: Some(80),
            viewport_width: Some(80),
            show_help_bar: true,
            show_scrollbar: false,
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new(&text).with_config(config);

        let rows = usize::from(TxtView::term_size().1);
        assert!(
            usize::from(v.visible_rows()) <= rows,
            "viewport {} exceeds terminal height {rows}",
            v.visible_rows()
        );

        let mut out = Vec::new();
        v.draw(&mut out).unwrap();
        assert!(
            max_move_to_row(&out) <= rows,
            "draw wrote past terminal height {rows}: {out:?}"
        );
    }

    #[test]
    fn draw_keeps_small_viewport_self_contained() {
        let config = TxtViewConfig {
            viewport_height: Some(5),
            viewport_width: Some(80),
            show_help_bar: true,
            show_scrollbar: false,
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new("a\nb\nc\nd\ne\nf").with_config(config);

        let mut out = Vec::new();
        v.draw(&mut out).unwrap();
        assert!(
            max_move_to_row(&out) <= 5,
            "draw escaped the 5-row viewport: {out:?}"
        );
    }

    #[test]
    fn help_bar_renders_on_narrow_terminal() {
        let config = TxtViewConfig {
            show_help_bar: true,
            show_scrollbar: false,
            viewport_height: Some(10),
            viewport_width: Some(1),
            ..TxtViewConfig::default()
        };
        let mut v = TxtView::new("a\nb\nc").with_config(config);
        let mut out = Vec::new();
        v.render_help_bar(&mut out, v.resolved_height()).unwrap();
        assert!(!out.is_empty(), "help bar must render on a narrow terminal");
        assert!(
            max_move_to_row(&out) <= 10,
            "help bar escaped the 10-row viewport: {out:?}"
        );
    }

    #[test]
    fn scrollbar_emits_thumb_blocks() {
        let mut v = viewer(25);
        assert!(v.max_offset > 0);
        let mut out = Vec::new();
        v.render_scrollbar(&mut out, usize::from(v.visible_rows()))
            .unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains('█'), "expected a thumb in output: {s:?}");
    }

    #[test]
    fn scrollbar_skipped_when_everything_fits() {
        let mut v = viewer(5);
        assert_eq!(v.max_offset, 0);
        let mut out = Vec::new();
        v.render_scrollbar(&mut out, usize::from(v.visible_rows()))
            .unwrap();
        assert!(out.is_empty(), "expected no output: {out:?}");
    }

    #[test]
    fn offset_from_thumb_top_is_monotonic_and_bounded() {
        let v = viewer(100);
        let visible = usize::from(v.visible_rows());
        let g = v.scroll_geometry(visible).expect("scrollbar present");
        let travel = i64::try_from(g.visible - g.size).unwrap_or(i64::MAX);

        assert_eq!(v.offset_from_thumb_top(-5, &g), 0);
        assert_eq!(v.offset_from_thumb_top(0, &g), 0);
        assert_eq!(v.offset_from_thumb_top(travel, &g), v.max_offset);

        let mut prev = 0;
        for top in 0..=travel {
            let off = v.offset_from_thumb_top(top, &g);
            assert!(off >= prev, "not monotonic at top={top}");
            assert!(off <= v.max_offset, "exceeds max_offset at top={top}");
            prev = off;
        }
    }

    #[test]
    fn dragging_keeps_thumb_on_mouse() {
        let mut v = viewer(100);
        let visible = v.visible_rows() as usize;
        let g = v.scroll_geometry(visible).unwrap();

        for mouse_y in 0..u16::try_from(visible).unwrap_or(u16::MAX) {
            let off = v.offset_from_thumb_top(i64::from(mouse_y), &g);
            v.offset = off;
            let moved = v.scroll_geometry(visible).unwrap();
            let travel = g.visible - g.size;
            let desired = usize::from(mouse_y).clamp(0, travel);
            assert!(
                moved.top.abs_diff(desired) <= 1,
                "thumb at {} dragged to {} for y={}",
                moved.top,
                desired,
                mouse_y
            );
        }
    }
}
