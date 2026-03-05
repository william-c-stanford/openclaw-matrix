pub mod widget;

/// Input state: text buffer and cursor position
pub struct InputState {
    pub text: String,
    pub cursor: usize,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let byte_pos = self.byte_offset();
        self.text.insert(byte_pos, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let byte_pos = self.byte_offset();
            let next = self.text[byte_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.text.drain(byte_pos..byte_pos + next);
        }
    }

    pub fn delete(&mut self) {
        let byte_pos = self.byte_offset();
        if byte_pos < self.text.len() {
            let next = self.text[byte_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.text.drain(byte_pos..byte_pos + next);
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        let char_count = self.text.chars().count();
        if self.cursor < char_count {
            self.cursor += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.text.chars().count();
    }

    pub fn take_text(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Convert character cursor to byte offset
    fn byte_offset(&self) -> usize {
        self.text
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }
}
