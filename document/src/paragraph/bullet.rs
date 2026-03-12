/// Unordered list bullet variant.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Bullet {
    Disc,
    Circle,
    Square,
    Custom(char),
}
