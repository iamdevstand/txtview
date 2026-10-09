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
        "a\r",
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
fn from_takes_over_string_and_cow_buffers() {
    let owned = String::from("x\ny");
    let from_string = TxtView::from(owned.clone());
    let via_into: TxtView = owned.clone().into();
    let from_owned_cow = TxtView::from(Cow::Owned(owned));
    let from_borrowed_cow = TxtView::from(Cow::Borrowed("x\ny"));
    for viewer in [from_string, via_into, from_owned_cow, from_borrowed_cow] {
        assert_eq!(viewer.line_count(), 2);
    }
}

#[test]
fn from_string_line_count_matches_str_lines_on_a_line_corpus() {
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
        "a\r",
        "\r\n",
        "a\r\n\r\nb",
        "one\ntwo\r\nthree\r\nfour\n\nfive",
        "🎉\nこんにちは\nx",
        "café\r\nnaïve",
        "👨\u{200d}👩\u{200d}👧\nline",
    ];
    for input in cases {
        let got = TxtView::from(input.to_string()).line_count();
        let expected = input.lines().count();
        assert_eq!(got, expected, "line count for {input:?}");
    }
}

#[test]
fn from_string_chains_with_config() {
    let text = String::from("one\ntwo\nthree");
    let viewer = TxtView::from(text).with_config(TxtViewConfig {
        show_line_numbers: true,
        ..TxtViewConfig::default()
    });
    assert_eq!(viewer.line_count(), 3);
    assert!(viewer.config().show_line_numbers);
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

#[test]
fn lines_matches_str_lines_on_a_line_corpus() {
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
        "a\r",
        "\r\n",
        "a\r\n\r\nb",
        "one\ntwo\r\nthree\r\nfour\n\nfive",
        "🎉\nこんにちは\nx",
        "café\r\nnaïve",
        "👨\u{200d}👩\u{200d}👧\nline",
    ];
    for input in cases {
        let viewer = TxtView::new(input);
        let got: Vec<&str> = viewer.lines().collect();
        let expected: Vec<&str> = input.lines().collect();
        assert_eq!(got, expected, "lines for {input:?}");
        assert_eq!(got.len(), viewer.line_count(), "line count for {input:?}");
    }
}

#[test]
fn as_ref_returns_the_document() {
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
        "a\r",
        "\r\n",
        "a\r\n\r\nb",
        "one\ntwo\r\nthree\r\nfour\n\nfive",
        "🎉\nこんにちは\nx",
        "café\r\nnaïve",
        "👨\u{200d}👩\u{200d}👧\nline",
    ];
    for input in cases {
        let via_new = TxtView::new(input);
        assert_eq!(via_new.as_ref(), input, "as_ref via new for {input:?}");
        let via_from = TxtView::from(input.to_string());
        assert_eq!(via_from.as_ref(), input, "as_ref via from for {input:?}");
    }
}
