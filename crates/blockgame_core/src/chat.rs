/// A contiguous segment of text sharing a single RGBA color.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorSpan {
    pub text: String,
    pub color: [f32; 4],
}

/// Maps a Minecraft-style hex color code character (`0`-`9`, `a`-`f`, case-insensitive)
/// to an RGBA color `[r, g, b, 1.0]`.
pub fn color_code_to_rgba(code: char) -> Option<[f32; 4]> {
    match code.to_ascii_lowercase() {
        '0' => Some([0.00, 0.00, 0.00, 1.0]), // Black
        '1' => Some([0.00, 0.00, 0.67, 1.0]), // Dark Blue
        '2' => Some([0.00, 0.67, 0.00, 1.0]), // Dark Green
        '3' => Some([0.00, 0.67, 0.67, 1.0]), // Dark Aqua
        '4' => Some([0.67, 0.00, 0.00, 1.0]), // Dark Red
        '5' => Some([0.67, 0.00, 0.67, 1.0]), // Dark Purple
        '6' => Some([1.00, 0.67, 0.00, 1.0]), // Gold
        '7' => Some([0.67, 0.67, 0.67, 1.0]), // Gray
        '8' => Some([0.33, 0.33, 0.33, 1.0]), // Dark Gray
        '9' => Some([0.33, 0.33, 1.00, 1.0]), // Blue
        'a' => Some([0.33, 1.00, 0.33, 1.0]), // Green
        'b' => Some([0.33, 1.00, 1.00, 1.0]), // Aqua / Cyan
        'c' => Some([1.00, 0.33, 0.33, 1.0]), // Red
        'd' => Some([1.00, 0.33, 1.00, 1.0]), // Light Purple
        'e' => Some([1.00, 1.00, 0.33, 1.0]), // Yellow
        'f' => Some([1.00, 1.00, 1.00, 1.0]), // White
        _ => None,
    }
}

/// Parses `&`-prefixed color codes (`&0`-`&9`, `&a`-`&f`, `&r` reset, `&&` literal `&`)
/// into colored text spans. Unknown `&x` sequences are preserved literally.
pub fn parse_colored_spans(input: &str, default_color: [f32; 4]) -> Vec<ColorSpan> {
    let mut spans = Vec::new();
    let mut current_color = default_color;
    let mut current_text = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '&' {
            if let Some(&next) = chars.peek() {
                if next == '&' {
                    chars.next();
                    current_text.push('&');
                    continue;
                }
                if next.eq_ignore_ascii_case(&'r') {
                    chars.next();
                    if !current_text.is_empty() {
                        spans.push(ColorSpan {
                            text: std::mem::take(&mut current_text),
                            color: current_color,
                        });
                    }
                    current_color = default_color;
                    continue;
                }
                if let Some(rgba) = color_code_to_rgba(next) {
                    chars.next();
                    if !current_text.is_empty() {
                        spans.push(ColorSpan {
                            text: std::mem::take(&mut current_text),
                            color: current_color,
                        });
                    }
                    // Preserve the alpha channel of `default_color` for chat fade-out
                    current_color = [rgba[0], rgba[1], rgba[2], default_color[3]];
                    continue;
                }
            }
        }
        current_text.push(ch);
    }

    if !current_text.is_empty() {
        spans.push(ColorSpan {
            text: current_text,
            color: current_color,
        });
    }

    spans
}

/// Strips valid `&`-prefixed color/reset codes and unescapes `&&` -> `&`.
pub fn strip_color_codes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '&' {
            if let Some(&next) = chars.peek() {
                if next == '&' {
                    chars.next();
                    out.push('&');
                    continue;
                }
                if next.eq_ignore_ascii_case(&'r') || color_code_to_rgba(next).is_some() {
                    chars.next();
                    continue;
                }
            }
        }
        out.push(ch);
    }

    out
}

/// Escapes `&` to `&&` so unpermitted players' `&` sequences render literally.
pub fn escape_color_codes(input: &str) -> String {
    input.replace('&', "&&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cyan_and_reset() {
        let default = [1.0, 1.0, 1.0, 0.8];
        let spans = parse_colored_spans("&bHello &rWorld", default);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "Hello ");
        assert_eq!(spans[0].color, [0.33, 1.0, 1.0, 0.8]);
        assert_eq!(spans[1].text, "World");
        assert_eq!(spans[1].color, default);
    }

    #[test]
    fn preserves_escaped_ampersand_and_unknown_codes() {
        let default = [1.0, 1.0, 1.0, 1.0];
        let spans = parse_colored_spans("AT&&T &ztest", default);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "AT&T &ztest");
    }

    #[test]
    fn strips_and_escapes() {
        assert_eq!(strip_color_codes("&c[Admin]&r Steve && Alex"), "[Admin] Steve & Alex");
        assert_eq!(escape_color_codes("&bhi"), "&&bhi");
    }
}
