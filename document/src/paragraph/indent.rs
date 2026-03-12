/// Paragraph indentation in points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Indent {
    /// Left margin in points.
    pub left: f32,
    /// Hanging indent in points (positive = text hangs past the bullet).
    pub hanging: f32,
}
