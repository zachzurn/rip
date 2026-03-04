/// Size level for text and header content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Size {
    /// Default body text (no markers)
    Text,
    /// Medium body text (`++ text ++`)
    TextM,
    /// Large body text (`+++ text +++`)
    TextL,
    /// Small header (`## text ##`)
    Title,
    /// Medium header (`### text ###`)
    TitleM,
    /// Large header (`#### text ####`)
    TitleL,
}

/// Inline text style applied to a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStyle {
    Normal,
    /// `*bold*`
    Bold,
    /// `_underline_`
    Underline,
    /// `` `italic` ``
    Italic,
    /// `~strikethrough~`
    Strikethrough,
}

/// A contiguous run of text with a single style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: SpanStyle,
}

/// Horizontal alignment within a pipe cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Content of a single cell — either styled text or an inline divider.
#[derive(Debug, Clone, PartialEq)]
pub enum CellContent {
    /// Inline text spans.
    Spans(Vec<Span>),
    /// Inline divider (`---`, `===`, `...` inside a pipe cell).
    Divider(LineStyle),
}

/// A single cell in a columnar layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub content: CellContent,
    pub align: Align,
}

/// Divider line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    /// `---` thin line
    Thin,
    /// `===` thick line
    Thick,
    /// `...` dotted line
    Dotted,
}

/// A single parsed node in the document.
///
/// The parser produces a flat `Vec<Node>` — no nesting.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// Plain or styled text line.
    Text {
        spans: Vec<Span>,
        size: Size,
    },

    /// Pipe-delimited columnar layout.
    Columns {
        cells: Vec<Cell>,
        size: Size,
    },

    /// Horizontal divider line.
    Line {
        style: LineStyle,
    },

    /// Empty line (line feed).
    Blank,

    /// `@printer-width(80mm)` — printer width (stored in mm after unit conversion).
    PrinterWidth { mm: f64 },

    /// `@printer-dpi(203)` — printer DPI.
    PrinterDpi { dpi: u32 },

    /// `@style(level, font, points)` — font and size assignment.
    Style {
        level: Size,
        font: String,
        points: f64,
    },

    /// `@image(url, width?, height?)` — inline image.
    Image {
        url: String,
        width: Option<f64>,
        height: Option<f64>,
        align: Option<Align>,
    },

    /// `@qr(data, size?)` — QR code.
    Qr {
        data: String,
        size: Option<f64>,
        align: Option<Align>,
    },

    /// `@barcode(format, data)` — barcode.
    Barcode {
        format: String,
        data: String,
    },

    /// `@cut()` or `@cut(partial)` — paper cut command.
    Cut {
        partial: bool,
    },

    /// `@feed(n)` — feed N blank lines.
    Feed {
        lines: u32,
    },

    /// `@drawer()` — open cash drawer.
    Drawer,
}
