//! The IOF control description symbols and their German and English texts. The symbol file, its reader and the
//! geometry live in freemapper-core's `crate::description_symbols` (core ADR 0017, ours 0049); this
//! module re-exports them under their old names and keeps the one function that takes the sheet language.

use crate::project::SheetLang;

pub use crate::description_symbols::{
    Circle, FLATNESS, MAX_DEPTH, MAX_POINTS_PER_SYMBOL, Standard, Stroke, SymbolGeometry, symbol,
};

/// The description text of symbol `id` in `lang`, with placeholders such as `{0}` left in.
pub fn symbol_text(id: &str, lang: SheetLang) -> Option<&'static str> {
    crate::description_symbols::symbol_text(
        id,
        match lang {
            SheetLang::De => crate::description_symbols::TextLang::De,
            SheetLang::En => crate::description_symbols::TextLang::En,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_in_de_and_en() {
        assert_eq!(symbol_text("1.1", SheetLang::De), Some("Terrasse"));
        assert_eq!(symbol_text("1.1", SheetLang::En), Some("terrace"));
        // Placeholders stay for the description box to fill.
        assert_eq!(symbol_text("11.7", SheetLang::De), Some("{0}, Biegung"));
    }

    #[test]
    fn known_symbol_geometry_comes_from_core() {
        let g = symbol("1.1", Standard::Iof2018).expect("1.1 exists");
        assert_eq!(g.kind, 'D');
        assert!(symbol("no-such-symbol", Standard::Iof2018).is_none());
    }
}
