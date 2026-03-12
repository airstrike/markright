/// Line spacing within a paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Spacing {
    /// Multiplier: 1.0 = single, 1.5, 2.0, etc.
    Multiple(f32),
    /// Fixed spacing in points.
    Exact(f32),
}
