/// Ordered list numbering variant.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Number {
    Arabic,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
}
