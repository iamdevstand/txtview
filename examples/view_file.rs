use std::{env, fs};

use txtview::TxtView;

fn main() -> std::io::Result<()> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| std::io::Error::other("usage: view_file <path>"))?;
    let text =
        fs::read_to_string(&path).map_err(|e| std::io::Error::other(format!("{}: {}", path, e)))?;

    let mut viewer = TxtView::new(&text);
    viewer.run()
}
