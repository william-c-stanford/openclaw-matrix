use crate::config::Charset;

pub const HALF_KATAKANA: &[char] = &[
    'ｦ', 'ｧ', 'ｨ', 'ｩ', 'ｪ', 'ｫ', 'ｬ', 'ｭ', 'ｮ', 'ｯ', 'ｰ', 'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ',
    'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ', 'ﾂ', 'ﾃ', 'ﾄ', 'ﾅ',
    'ﾆ', 'ﾇ', 'ﾈ', 'ﾉ', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ', 'ﾔ', 'ﾕ',
    'ﾖ', 'ﾗ', 'ﾘ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾜ', 'ﾝ',
];

pub const DIGITS: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

pub const ASCII: &[char] = &[
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
    'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    '!', '@', '#', '$', '%', '&', '*', '+', '-', '=', '<', '>', '/',
];

#[allow(dead_code)]
pub fn char_pool(charset: Charset) -> &'static [char] {
    match charset {
        Charset::Default => {
            // We'll use a combined approach: random_char handles this
            HALF_KATAKANA
        }
        Charset::Katakana => HALF_KATAKANA,
        Charset::Ascii => ASCII,
        Charset::Digits => DIGITS,
    }
}

pub fn random_char(rng: &mut fastrand::Rng, charset: Charset) -> char {
    match charset {
        Charset::Default => {
            // Mix katakana + digits (classic Matrix look)
            let total = HALF_KATAKANA.len() + DIGITS.len();
            let idx = rng.usize(..total);
            if idx < HALF_KATAKANA.len() {
                HALF_KATAKANA[idx]
            } else {
                DIGITS[idx - HALF_KATAKANA.len()]
            }
        }
        Charset::Katakana => HALF_KATAKANA[rng.usize(..HALF_KATAKANA.len())],
        Charset::Ascii => ASCII[rng.usize(..ASCII.len())],
        Charset::Digits => DIGITS[rng.usize(..DIGITS.len())],
    }
}
