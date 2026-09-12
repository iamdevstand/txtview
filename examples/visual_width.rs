use txtview::TxtView;

fn main() {
    let lines = vec![
        "Visual width demo".to_string(),
        "".to_string(),
        "CJK characters take 2 columns each:".to_string(),
        "  中文测试 ABC abc 123".to_string(),
        "  日本語テスト ABC abc 123".to_string(),
        "  한국어테스트 ABC abc 123".to_string(),
        "".to_string(),
        "Mixed ASCII and CJK:".to_string(),
        "  Hello你好World世界!".to_string(),
        "  Helloあなたは元気です!".to_string(),
        "  Hello안녕하세요!".to_string(),
        "  Price: ¥100 ($15 USD)".to_string(),
        "".to_string(),
        "Full-width punctuation:".to_string(),
        "  「引用符」《书名号》【括号】".to_string(),
        "".to_string(),
        "Emoji (often 2 columns):".to_string(),
        "  Hello 👋 World 🌍".to_string(),
        "".to_string(),
        "Wrapping counts visual columns, not characters:".to_string(),
        "  12345678901234567890 (20 chars, 20 cols)".to_string(),
        "  一二三四五六七八九十 (10 chars, 20 cols)".to_string(),
        "".to_string(),
        "Both lines above wrap at 20 columns:".to_string(),
        "  一二三四五六七八九十十一二 (12 CJK chars, 24 cols)".to_string(),
        "".to_string(),
        "24 CJK chars = 48 cols, wraps into rows at column boundaries:".to_string(),
        "  一二三四五六七八九十".to_string(),
        "  一二三四五六七八九十".to_string(),
        "  一二三四五六七八九十".to_string(),
    ];

    let mut viewer = TxtView::new(lines.join("\n"));
    viewer.run().unwrap();
}
