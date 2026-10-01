//! Shift_JIS text (old ANSI text records, file names in properties).

pub fn decode(b: &[u8]) -> String {
    let b = match b.iter().position(|&c| c == 0) {
        Some(n) => &b[..n],
        None => b,
    };
    encoding_rs::SHIFT_JIS.decode_without_bom_handling(b).0.into_owned()
}

pub fn encode(s: &str) -> Vec<u8> {
    encoding_rs::SHIFT_JIS.encode(s).0.into_owned()
}
