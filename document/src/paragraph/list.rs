use super::bullet;
use super::number;

/// List marker style for a paragraph.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum List {
    Bullet(bullet::Bullet),
    Ordered(number::Number),
}
