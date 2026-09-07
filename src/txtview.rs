mod layout;
mod navigation;
mod render;
mod run;

use crate::TxtViewConfig;

pub struct TxtView {
    lines: Vec<String>,
    display: Vec<String>,
    rows_per_line: Vec<usize>,
    offset: usize,
    max_offset: usize,
    config: TxtViewConfig,
}

impl TxtView {
    pub fn new(input: &str) -> Self {
        let lines: Vec<String> = input.lines().map(String::from).collect();
        let mut view = TxtView {
            rows_per_line: vec![1; lines.len()],
            lines,
            display: Vec::new(),
            offset: 0,
            max_offset: 0,
            config: TxtViewConfig::default(),
        };
        view.refresh_bounds();
        view
    }

    pub fn with_config(mut self, config: TxtViewConfig) -> Self {
        self.config = config;
        self.refresh_bounds();
        self
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn max_offset(&self) -> usize {
        self.max_offset
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}
