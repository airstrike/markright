// Re-exports from iced
pub use iced_core::text::rich_editor::paragraph::{
    Border, Borders, Bullet, Fill, Geometry, Indent, List, Number, Spacing, Style,
};

// --- markright's semantic layer ---

/// An opaque, interned paragraph name.
///
/// Ten well-known constants cover the common paragraph types. Unknown
/// slugs are interned via a global cache (same pattern as iced's
/// `Family::name`) — one leak per unique slug, unbounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Name(&'static str);

impl Name {
    pub const BODY: Name = Name("body");
    pub const HEADING_1: Name = Name("heading-1");
    pub const HEADING_2: Name = Name("heading-2");
    pub const HEADING_3: Name = Name("heading-3");
    pub const HEADING_4: Name = Name("heading-4");
    pub const HEADING_5: Name = Name("heading-5");
    pub const HEADING_6: Name = Name("heading-6");
    pub const CODE_BLOCK: Name = Name("code-block");
    pub const BLOCK_QUOTE: Name = Name("block-quote");
    pub const RULE: Name = Name("rule");

    pub const fn heading(level: u8) -> Self {
        match level {
            1 => Self::HEADING_1,
            2 => Self::HEADING_2,
            3 => Self::HEADING_3,
            4 => Self::HEADING_4,
            5 => Self::HEADING_5,
            6 => Self::HEADING_6,
            _ => Self::BODY,
        }
    }

    pub fn heading_level(&self) -> Option<u8> {
        match self.0 {
            "heading-1" => Some(1),
            "heading-2" => Some(2),
            "heading-3" => Some(3),
            "heading-4" => Some(4),
            "heading-5" => Some(5),
            "heading-6" => Some(6),
            _ => None,
        }
    }

    pub fn from_slug(slug: &str) -> Self {
        match slug {
            "body" => return Self::BODY,
            "heading-1" => return Self::HEADING_1,
            "heading-2" => return Self::HEADING_2,
            "heading-3" => return Self::HEADING_3,
            "heading-4" => return Self::HEADING_4,
            "heading-5" => return Self::HEADING_5,
            "heading-6" => return Self::HEADING_6,
            "code-block" => return Self::CODE_BLOCK,
            "block-quote" => return Self::BLOCK_QUOTE,
            "rule" => return Self::RULE,
            _ => {}
        }
        // Intern cache (iced Family::name pattern)
        use std::collections::HashSet;
        use std::sync::{LazyLock, Mutex};
        static INTERN: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Mutex::default);
        let mut cache = INTERN.lock().expect("Name intern lock poisoned");
        if let Some(existing) = cache.get(slug) {
            return Name(existing);
        }
        let interned: &'static str = slug.to_owned().leak();
        cache.insert(interned);
        Name(interned)
    }

    pub const fn as_slug(&self) -> &'static str {
        self.0
    }
}

impl Default for Name {
    fn default() -> Self {
        Self::BODY
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

// --- OverrideSet ---

/// Bitflags tracking which paragraph fields the user has explicitly set.
///
/// When changing a paragraph's name, `Theme::apply` preserves fields
/// whose override bit is set, so user customizations survive name changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OverrideSet(u32);

impl OverrideSet {
    pub const NONE: Self = Self(0);
    pub const ALIGNMENT: Self = Self(1 << 0);
    pub const SPACE_BEFORE: Self = Self(1 << 1);
    pub const SPACING_AFTER: Self = Self(1 << 2);
    pub const LINE_HEIGHT: Self = Self(1 << 3);
    pub const LINE_SPACING: Self = Self(1 << 4);
    pub const LEVEL: Self = Self(1 << 5);
    pub const LIST: Self = Self(1 << 6);
    pub const INDENT: Self = Self(1 << 7);
    pub const FILL: Self = Self(1 << 8);
    pub const BORDERS: Self = Self(1 << 9);
    pub const BOLD: Self = Self(1 << 10);
    pub const ITALIC: Self = Self(1 << 11);
    pub const UNDERLINE: Self = Self(1 << 12);
    pub const STRIKETHROUGH: Self = Self(1 << 13);
    pub const FONT: Self = Self(1 << 14);
    pub const SIZE: Self = Self(1 << 15);
    pub const COLOR: Self = Self(1 << 16);
    pub const LETTER_SPACING: Self = Self(1 << 17);
    pub const CONTIGUOUS: Self = Self(1 << 18);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn clear(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

// --- Paragraph ---

/// A paragraph with a semantic name, visual style, and override tracking.
///
/// Wraps iced's `paragraph::Style` with markright's naming system.
/// iced calls receive `&paragraph::Style` (unwrapped at the boundary).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Paragraph {
    pub name: Name,
    pub style: Style,
    overrides: OverrideSet,
}

impl Paragraph {
    pub fn new(name: Name, style: Style) -> Self {
        Self {
            name,
            style,
            overrides: OverrideSet::NONE,
        }
    }

    pub fn body() -> Self {
        Self::default()
    }

    pub fn overrides(&self) -> OverrideSet {
        self.overrides
    }

    pub fn set_override(&mut self, flag: OverrideSet) {
        self.overrides = self.overrides.union(flag);
    }

    pub fn clear_overrides(&mut self) {
        self.overrides = OverrideSet::NONE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_default_is_body() {
        assert_eq!(Name::default(), Name::BODY);
    }

    #[test]
    fn heading_roundtrip() {
        for level in 1..=6 {
            let name = Name::heading(level);
            assert_eq!(name.heading_level(), Some(level));
        }
    }

    #[test]
    fn heading_out_of_range_is_body() {
        assert_eq!(Name::heading(0), Name::BODY);
        assert_eq!(Name::heading(7), Name::BODY);
    }

    #[test]
    fn non_heading_has_no_level() {
        assert_eq!(Name::BODY.heading_level(), None);
        assert_eq!(Name::CODE_BLOCK.heading_level(), None);
        assert_eq!(Name::RULE.heading_level(), None);
    }

    #[test]
    fn from_slug_known_names() {
        assert_eq!(Name::from_slug("body"), Name::BODY);
        assert_eq!(Name::from_slug("heading-3"), Name::HEADING_3);
        assert_eq!(Name::from_slug("code-block"), Name::CODE_BLOCK);
        assert_eq!(Name::from_slug("block-quote"), Name::BLOCK_QUOTE);
        assert_eq!(Name::from_slug("rule"), Name::RULE);
    }

    #[test]
    fn from_slug_unknown_is_interned() {
        let a = Name::from_slug("custom-style");
        let b = Name::from_slug("custom-style");
        assert_eq!(a, b);
        assert_eq!(a.as_slug(), "custom-style");
    }

    #[test]
    fn override_set_operations() {
        let mut flags = OverrideSet::NONE;
        assert!(flags.is_empty());
        assert!(!flags.contains(OverrideSet::SIZE));

        flags = flags.union(OverrideSet::SIZE);
        assert!(flags.contains(OverrideSet::SIZE));
        assert!(!flags.contains(OverrideSet::BOLD));

        flags = flags.union(OverrideSet::BOLD);
        assert!(flags.contains(OverrideSet::SIZE));
        assert!(flags.contains(OverrideSet::BOLD));

        flags = flags.clear(OverrideSet::SIZE);
        assert!(!flags.contains(OverrideSet::SIZE));
        assert!(flags.contains(OverrideSet::BOLD));
    }

    #[test]
    fn paragraph_default_is_body() {
        let p = Paragraph::default();
        assert_eq!(p.name, Name::BODY);
        assert!(p.overrides().is_empty());
    }

    #[test]
    fn paragraph_override_tracking() {
        let mut p = Paragraph::body();
        p.style.style.size = Some(40.0);
        p.set_override(OverrideSet::SIZE);
        assert!(p.overrides().contains(OverrideSet::SIZE));

        p.clear_overrides();
        assert!(p.overrides().is_empty());
    }

    #[test]
    fn name_display() {
        assert_eq!(format!("{}", Name::HEADING_1), "heading-1");
        assert_eq!(format!("{}", Name::CODE_BLOCK), "code-block");
    }
}
