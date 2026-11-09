//! Why a sheet note was given, as values the CLI and the MCP server turn into text in the
//! program language (ADR 0024). `Display` gives the English text for logs and tests; details
//! taken from a library or the operating system (`error`, `what`) stay as they come.

use std::fmt;

/// Why a logo is left out ([`SheetNote::LogoSkipped`](super::SheetNote::LogoSkipped)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogoSkipReason {
    /// The element lacks `file` or `width_mm` (only for an element built by hand: the project
    /// loader refuses such an element).
    NeedsFileAndWidth,
    NotFound,
    /// The operating system could not read the file; `error` is its message.
    Unreadable {
        error: String,
    },
    /// A folder, a device or a pipe.
    NotAFile,
    /// Larger than 10 MB.
    FileTooLarge,
    /// Neither PNG nor JPEG nor SVG.
    UnknownFormat,
    /// More than 25 megapixels.
    TooManyPixels {
        width: u32,
        height: u32,
    },
    /// Wider or higher than 65 535 pixels.
    TooWideOrHigh {
        width: u32,
        height: u32,
    },
    /// The image decoder failed; `error` is its message.
    Undecodable {
        error: String,
    },
    /// The decoder would need more memory than a 25 megapixel image in 8 bits.
    DecoderLimit,
    SvgNotUtf8,
    /// The XML parser or `usvg` failed; `error` is their message or where the XML breaks.
    SvgUnreadable {
        error: String,
    },
    /// The SVG uses something the allow-list does not take (ADR 0026); `what` names it.
    SvgUnsupported {
        what: String,
    },
    /// The SVG breaks a limit on its size (ADR 0019); `what` names which.
    SvgTooComplex {
        what: String,
    },
    /// The logo has no usable width or height.
    CannotSize,
    /// The sheet's raster logos reached 50 megapixels (ADR 0026).
    PixelBudget,
    /// The sheet's SVG logos reached 1 000 000 units of work (ADR 0026).
    SvgBudget,
}

impl fmt::Display for LogoSkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use LogoSkipReason::*;
        match self {
            NeedsFileAndWidth => f.write_str("a logo needs `file` and `width_mm`"),
            NotFound => f.write_str("not found"),
            Unreadable { error } => write!(f, "cannot read the file ({error})"),
            NotAFile => f.write_str("not a regular file"),
            FileTooLarge => f.write_str("larger than 10 MB"),
            UnknownFormat => f.write_str("not a PNG, JPEG or SVG file"),
            TooManyPixels { width, height } => {
                write!(f, "{width} x {height} pixels, more than 25 megapixels")
            }
            TooWideOrHigh { width, height } => {
                write!(f, "{width} x {height} pixels, wider or higher than 65 535")
            }
            Undecodable { error } => write!(f, "cannot decode the image ({error})"),
            DecoderLimit => {
                f.write_str("too large to decode; save it as an 8-bit image or with fewer pixels")
            }
            SvgNotUtf8 => f.write_str("cannot read the SVG (not UTF-8)"),
            SvgUnreadable { error } => write!(f, "cannot read the SVG ({error})"),
            SvgUnsupported { what } => write!(
                f,
                "{what} is not supported in logos; export the logo as a plain SVG (shapes, \
                 paths, gradients)"
            ),
            SvgTooComplex { what } => f.write_str(what),
            CannotSize => f.write_str("cannot size the logo"),
            PixelBudget => f.write_str("more than 50 megapixels of logos on this sheet"),
            SvgBudget => {
                f.write_str("more SVG logo work on this sheet than its budget of 1 000 000 units")
            }
        }
    }
}

/// How an SVG logo is drawn simpler than written
/// ([`SheetNote::LogoSimplified`](super::SheetNote::LogoSimplified)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogoSimplification {
    /// A gradient is drawn in its first stop colour.
    Gradient,
    /// A pattern fill is drawn in black.
    Pattern,
    /// Transparency is ignored; drawn opaque.
    Opacity,
    /// Dashes are drawn as solid lines.
    Dashes,
    /// An embedded image is left out.
    Image,
    /// Text is left out.
    Text,
    /// A clip path is ignored.
    ClipPath,
}

impl fmt::Display for LogoSimplification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LogoSimplification::Gradient => "gradient drawn in its first stop colour",
            LogoSimplification::Pattern => "pattern fill drawn in black",
            LogoSimplification::Opacity => "transparency ignored, drawn opaque",
            LogoSimplification::Dashes => "dashes drawn as solid lines",
            LogoSimplification::Image => "embedded image left out",
            LogoSimplification::Text => "text left out (convert it to paths)",
            LogoSimplification::ClipPath => "clip path ignored",
        })
    }
}

/// What a layout lacks for the print scale and the contour interval
/// ([`SheetNote::MissingScaleOrContour`](super::SheetNote::MissingScaleOrContour)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingItem {
    /// No text of the layout shows `{print_scale}`.
    PrintScaleText,
    /// No text of the layout shows `{contour_interval}`.
    ContourText,
    /// `map.contour_interval_m` is not set.
    ContourValue,
}

impl MissingItem {
    /// The placeholder or field it is about, as the user writes it.
    pub fn name(self) -> &'static str {
        match self {
            MissingItem::PrintScaleText => "{print_scale}",
            MissingItem::ContourText => "{contour_interval}",
            MissingItem::ContourValue => "map.contour_interval_m",
        }
    }
}
