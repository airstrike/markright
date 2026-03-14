/// A trait extension for ternary functions with a reference as first parameter
/// (`Fn(&T, B, C) -> O`).
pub trait Ternary<T, B, C, O>: Sized {
    /// Applies the third argument to a ternary function and returns
    /// a new function that takes a reference and second argument.
    fn with(self, last: C) -> impl Fn(&T, B) -> O
    where
        C: Clone;
}

impl<F, T, B, C, O> Ternary<T, B, C, O> for F
where
    F: Fn(&T, B, C) -> O,
{
    fn with(self, last: C) -> impl Fn(&T, B) -> O
    where
        C: Clone,
    {
        move |t, b| self(t, b, last.clone())
    }
}
