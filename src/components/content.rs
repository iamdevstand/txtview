use std::io;

use crossterm::{QueueableCommand, cursor::MoveTo};

use crate::surface::{Area, Component};
use crate::text::write_visible_row;

/// The scrollable text rows of the viewer.
///
/// Content is the leftover piece: the other pieces are placed in the frame
/// first and content is granted whatever remains of it. It fills every
/// granted row, blank past the document end, anchoring to the area it is
/// handed.
pub(crate) struct Content<'a> {
    display: &'a [String],
    offset: usize,
}

impl<'a> Content<'a> {
    pub(crate) fn new(display: &'a [String], offset: usize) -> Self {
        Content { display, offset }
    }
}

impl Component for Content<'_> {
    fn area(&self, boxed: Area) -> Area {
        boxed
    }

    fn render(&self, area: Area, out: &mut dyn io::Write) -> io::Result<()> {
        if area.is_empty() {
            return Ok(());
        }
        for i in 0..area.height {
            let text = self
                .display
                .get(self.offset + usize::from(i))
                .map(String::as_str)
                .unwrap_or("");
            let (col, row) = (area.col, area.row + i);
            QueueableCommand::queue(out, MoveTo(col, row))?;
            // Write the leading text that fits the area, skipping a trailing
            // cluster wider than the area rather than splitting it, then pad
            // the remainder with spaces so stale cells fade without ever
            // writing past the cells the content owns
            let used = write_visible_row(text, usize::from(area.width), out)?;
            let pad = usize::from(area.width).saturating_sub(used);
            if pad > 0 {
                write!(out, "{}", " ".repeat(pad))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(text: &[&str], offset: usize, area: Area) -> String {
        let display: Vec<String> = text.iter().map(|s| s.to_string()).collect();
        let mut out = Vec::new();
        Content::new(&display, offset)
            .render(area, &mut out)
            .unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn renders_only_the_requested_rows() {
        let out = render(&["first", "second", "third"], 0, box_height(2));
        assert!(out.contains("first"));
        assert!(out.contains("second"));
        assert!(!out.contains("third"));
    }

    #[test]
    fn renders_blank_past_the_document_end() {
        let out = render(&["only"], 0, box_height(3));
        assert!(out.contains("only"));
    }

    #[test]
    fn honors_the_scroll_offset() {
        let out = render(&["a", "b", "c"], 1, box_height(2));
        assert!(out.contains("b"));
        assert!(out.contains("c"));
        assert!(!out.contains("a"));
    }

    #[test]
    fn draws_inside_its_area() {
        let out = render(&["hi"], 0, box_at(4, 3, 20, 1));
        assert!(
            out.contains("\x1b[4;5H"),
            "content must anchor to the area origin: {out:?}"
        );
    }

    #[test]
    fn pads_rows_to_the_area_width_instead_of_clearing_the_line() {
        let out = render(&["ab"], 0, box_at(0, 0, 3, 1));
        assert!(out.contains("ab "), "row padded to 3 columns: {out:?}");
        assert!(!out.contains("\x1b[K"), "no whole-line clear: {out:?}");
    }

    #[test]
    fn fills_empty_lines_with_spaces() {
        let out = render(&["only"], 1, box_at(0, 0, 5, 1));
        assert!(
            out.contains("     "),
            "empty line padded to 5 columns: {out:?}"
        );
    }

    #[test]
    fn renders_nothing_in_an_empty_area() {
        let out = render(&["hidden"], 0, box_at(0, 0, 0, 5));
        assert!(out.is_empty(), "a zero-width area must not paint: {out:?}");
        let out = render(&["hidden"], 0, box_at(0, 0, 5, 0));
        assert!(out.is_empty(), "a zero-height area must not paint: {out:?}");
    }

    #[test]
    fn clips_a_cluster_wider_than_the_area_whole() {
        let out = render(&["🎉"], 0, box_at(0, 0, 1, 1));
        assert_eq!(
            out, "\x1b[1;1H ",
            "a 2-wide emoji in a 1-column area must pad, not escape"
        );
        let out = render(&["ab🎉"], 0, box_at(0, 0, 3, 1));
        assert_eq!(
            out, "\x1b[1;1Hab ",
            "the trailing cluster is clipped whole and the pad fills the edge"
        );
    }

    fn box_height(rows: u16) -> Area {
        Area {
            col: 0,
            row: 0,
            width: 20,
            height: rows,
        }
    }

    fn box_at(col: u16, row: u16, width: u16, height: u16) -> Area {
        Area {
            col,
            row,
            width,
            height,
        }
    }
}
