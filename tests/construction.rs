use std::borrow::Cow;

use txtview::{TxtView, TxtViewConfig};

mod common;

#[test]
fn line_count_matches_str_lines_on_a_line_corpus() {
    let cases = [
        "",
        "a",
        "a\nb",
        "a\n",
        "\n",
        "\n\n",
        "a\n\nb",
        "a\r\nb",
        "a\rb",
        "\r\n",
        "a\r\n\r\nb",
        "one\ntwo\r\nthree\r\nfour\n\nfive",
        "🎉\nこんにちは\nx",
        "café\r\nnaïve",
        "👨\u{200d}👩\u{200d}👧\nline",
    ];
    for input in cases {
        let got = TxtView::new(input).line_count();
        let expected = input.lines().count();
        assert_eq!(got, expected, "line count for {input:?}");
    }
}

#[test]
fn new_accepts_some_as_ref_str_implementor() {
    let owned = String::from("x\ny");
    let borrowed_str = TxtView::new("x\ny");
    let owned_string = TxtView::new(owned.clone());
    let re_borrowed = TxtView::new(&owned);
    let borrowed_cow = TxtView::new(Cow::Borrowed("x\ny"));
    let owned_cow = TxtView::new(Cow::Owned(owned));
    for viewer in [
        borrowed_str,
        owned_string,
        re_borrowed,
        borrowed_cow,
        owned_cow,
    ] {
        assert_eq!(viewer.line_count(), 2);
    }
}

#[test]
fn typical_downstream_usage() {
    let config = TxtViewConfig {
        show_line_numbers: true,
        ..TxtViewConfig::default()
    };
    let viewer = TxtView::new("one\ntwo\nthree").with_config(config);
    assert_eq!(viewer.line_count(), 3);
    assert!(viewer.config().show_line_numbers);
}

#[test]
fn default_constructed_viewer_reports_default_config() {
    let viewer = TxtView::new("x");
    common::assert_default_config(viewer.config());
}
