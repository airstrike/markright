/// Heading level options for the pick list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingOption {
    Normal,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl HeadingOption {
    /// All heading options for populating a pick list.
    pub const ALL: [HeadingOption; 7] = [
        HeadingOption::Normal,
        HeadingOption::H1,
        HeadingOption::H2,
        HeadingOption::H3,
        HeadingOption::H4,
        HeadingOption::H5,
        HeadingOption::H6,
    ];

    /// Convert from an optional heading level (as stored in the document).
    pub fn from_level(level: Option<u8>) -> Self {
        match level {
            None => Self::Normal,
            Some(1) => Self::H1,
            Some(2) => Self::H2,
            Some(3) => Self::H3,
            Some(4) => Self::H4,
            Some(5) => Self::H5,
            Some(6) => Self::H6,
            Some(_) => Self::Normal,
        }
    }

    /// Convert to an optional heading level.
    pub fn to_level(self) -> Option<u8> {
        match self {
            Self::Normal => None,
            Self::H1 => Some(1),
            Self::H2 => Some(2),
            Self::H3 => Some(3),
            Self::H4 => Some(4),
            Self::H5 => Some(5),
            Self::H6 => Some(6),
        }
    }
}

impl std::fmt::Display for HeadingOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "Normal"),
            Self::H1 => write!(f, "Heading 1"),
            Self::H2 => write!(f, "Heading 2"),
            Self::H3 => write!(f, "Heading 3"),
            Self::H4 => write!(f, "Heading 4"),
            Self::H5 => write!(f, "Heading 5"),
            Self::H6 => write!(f, "Heading 6"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_option_round_trips() {
        for option in HeadingOption::ALL {
            let level = option.to_level();
            let back = HeadingOption::from_level(level);
            assert_eq!(option, back);
        }
    }

    #[test]
    fn heading_option_display() {
        assert_eq!(HeadingOption::Normal.to_string(), "Normal");
        assert_eq!(HeadingOption::H1.to_string(), "Heading 1");
        assert_eq!(HeadingOption::H6.to_string(), "Heading 6");
    }

    #[test]
    fn heading_option_from_unknown_level_is_normal() {
        assert_eq!(HeadingOption::from_level(Some(7)), HeadingOption::Normal);
        assert_eq!(HeadingOption::from_level(Some(0)), HeadingOption::Normal);
    }
}
