use crate::ast::{Span, SpanStyle};

/// Returns the style associated with a delimiter character, if any.
fn delimiter_style(ch: char) -> Option<SpanStyle> {
    match ch {
        '*' => Some(SpanStyle::Bold),
        '_' => Some(SpanStyle::Underline),
        '`' => Some(SpanStyle::Italic),
        '~' => Some(SpanStyle::Strikethrough),
        _ => None,
    }
}

/// Parse inline styled spans from text content.
///
/// Handles `*bold*`, `_underline_`, `` `italic` ``, `~strikethrough~`, and `\` escaping.
/// If an opening delimiter has no matching close, the style applies to the rest of the text.
pub fn parse_spans(input: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut chars = input.chars().peekable();
    let mut buf = String::new();
    let mut current_style = SpanStyle::Normal;

    while let Some(&ch) = chars.peek() {
        // Escape: backslash consumes next char literally
        if ch == '\\' {
            chars.next(); // consume '\'
            if let Some(&next) = chars.peek() {
                buf.push(next);
                chars.next();
            }
            continue;
        }

        // Check for style delimiter
        if let Some(style) = delimiter_style(ch) {
            if current_style == SpanStyle::Normal {
                // Opening a new style — flush the normal buffer
                if !buf.is_empty() {
                    spans.push(Span {
                        text: buf.clone(),
                        style: SpanStyle::Normal,
                    });
                    buf.clear();
                }
                current_style = style;
                chars.next(); // consume opening delimiter
            } else if current_style == style {
                // Closing the current style — flush the styled buffer
                if !buf.is_empty() {
                    spans.push(Span {
                        text: buf.clone(),
                        style: current_style,
                    });
                    buf.clear();
                }
                current_style = SpanStyle::Normal;
                chars.next(); // consume closing delimiter
            } else {
                // Different style delimiter while inside a style — treat as literal
                buf.push(ch);
                chars.next();
            }
        } else {
            buf.push(ch);
            chars.next();
        }
    }

    // Flush remaining buffer — if a style was opened but never closed,
    // the style applies to the rest of the text (start/end rule).
    if !buf.is_empty() {
        spans.push(Span {
            text: buf,
            style: current_style,
        });
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn span(text: &str, style: SpanStyle) -> Span {
        Span {
            text: text.to_string(),
            style,
        }
    }

    #[test]
    fn plain_text() {
        assert_eq!(
            parse_spans("hello world"),
            vec![span("hello world", SpanStyle::Normal)]
        );
    }

    #[test]
    fn bold() {
        assert_eq!(
            parse_spans("*bold*"),
            vec![span("bold", SpanStyle::Bold)]
        );
    }

    #[test]
    fn underline() {
        assert_eq!(
            parse_spans("_underline_"),
            vec![span("underline", SpanStyle::Underline)]
        );
    }

    #[test]
    fn italic() {
        assert_eq!(
            parse_spans("`italic`"),
            vec![span("italic", SpanStyle::Italic)]
        );
    }

    #[test]
    fn strikethrough() {
        assert_eq!(
            parse_spans("~strikethrough~"),
            vec![span("strikethrough", SpanStyle::Strikethrough)]
        );
    }

    #[test]
    fn mixed_text_and_style() {
        assert_eq!(
            parse_spans("hello *world*"),
            vec![
                span("hello ", SpanStyle::Normal),
                span("world", SpanStyle::Bold),
            ]
        );
    }

    #[test]
    fn style_in_middle() {
        assert_eq!(
            parse_spans("before *bold* after"),
            vec![
                span("before ", SpanStyle::Normal),
                span("bold", SpanStyle::Bold),
                span(" after", SpanStyle::Normal),
            ]
        );
    }

    #[test]
    fn unclosed_style_applies_to_rest() {
        assert_eq!(
            parse_spans("*bold to the end"),
            vec![span("bold to the end", SpanStyle::Bold)]
        );
    }

    #[test]
    fn escaped_delimiter() {
        assert_eq!(
            parse_spans("\\*not bold\\*"),
            vec![span("*not bold*", SpanStyle::Normal)]
        );
    }

    #[test]
    fn escaped_backslash() {
        assert_eq!(
            parse_spans("back\\\\slash"),
            vec![span("back\\slash", SpanStyle::Normal)]
        );
    }

    #[test]
    fn nested_delimiters_literal() {
        // Different delimiter inside a style is treated as literal text
        assert_eq!(
            parse_spans("*bold _with_ under*"),
            vec![span("bold _with_ under", SpanStyle::Bold)]
        );
    }

    #[test]
    fn multiple_styles_sequential() {
        assert_eq!(
            parse_spans("*bold* and _underline_"),
            vec![
                span("bold", SpanStyle::Bold),
                span(" and ", SpanStyle::Normal),
                span("underline", SpanStyle::Underline),
            ]
        );
    }

    #[test]
    fn empty_input() {
        assert_eq!(parse_spans(""), Vec::<Span>::new());
    }

    #[test]
    fn style_at_boundaries() {
        assert_eq!(
            parse_spans("*TOTAL*"),
            vec![span("TOTAL", SpanStyle::Bold)]
        );
    }

    #[test]
    fn dollar_amounts_with_style() {
        assert_eq!(
            parse_spans("*$19.74*"),
            vec![span("$19.74", SpanStyle::Bold)]
        );
    }
}
