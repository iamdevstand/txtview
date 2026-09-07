use txtview::TxtView;

#[test]
fn new_splits_input_into_lines() {
    let v = TxtView::new("foo\nbar\nbaz");
    assert_eq!(v.line_count(), 3);
}

#[test]
fn new_handles_empty_input() {
    let v = TxtView::new("");
    assert_eq!(v.line_count(), 0);
}

#[test]
fn new_preserves_blank_lines() {
    let v = TxtView::new("a\n\nb");
    assert_eq!(v.line_count(), 3);
}

#[test]
fn new_ignores_trailing_newline() {
    let v = TxtView::new("a\nb\n");
    assert_eq!(v.line_count(), 2);
}
