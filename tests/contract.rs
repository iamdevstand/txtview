use std::process::{Command, Stdio};

use txtview::{TxtView, TxtViewConfig};

const FORCE_NON_TTY: &str = "TXTVIEW_TEST_FORCE_NON_TTY";

#[test]
fn viewer_and_config_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TxtView>();
    assert_send_sync::<TxtViewConfig>();
}

#[test]
fn run_errors_when_stdout_is_not_a_terminal() {
    if std::env::var_os(FORCE_NON_TTY).is_some() {
        let err = TxtView::new("hello\nworld").run().unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotConnected);
        return;
    }
    let binary = std::env::current_exe().expect("test binary path");
    let out = Command::new(binary)
        .args(["--exact", "run_errors_when_stdout_is_not_a_terminal"])
        .env(FORCE_NON_TTY, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("run test harness");
    assert!(
        out.status.success(),
        "child with piped stdout must get ErrorKind::NotConnected"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("1 passed") && stdout.contains("run_errors_when_stdout_is_not_a_terminal"),
        "child must have run the test itself, got: {stdout}"
    );
}

#[test]
fn clone_keeps_line_count_and_config() {
    let original =
        TxtView::new("a\nb\nc").with_config(TxtViewConfig::default().with_show_line_numbers(true));
    let cloned = original.clone();
    assert_eq!(cloned.line_count(), original.line_count());
    assert!(cloned.config().show_line_numbers);
    assert!(original.config().show_line_numbers);
}

#[test]
fn debug_describes_metadata_without_leaking_the_document() {
    let viewer = TxtView::new("super-secret document\nline two");
    let out = format!("{viewer:?}");
    assert!(out.contains("line_count:"), "must report line count: {out}");
    assert!(
        !out.contains("super-secret"),
        "must not dump the document: {out}"
    );
}
