//! `Lent<T>`: a field that can be lent out whole for a while (`AppState::view`, lent to the views
//! for a frame by `AppState::with_view`). It derefs to the value; while lent it holds a default
//! in its place, and reaching it then is a bug (the borrower has the real one): debug builds
//! panic, release builds read the default.

use std::ops::{Deref, DerefMut};

#[derive(Default)]
pub struct Lent<T> {
    value: T,
    lent: bool,
}

impl<T: Default> Lent<T> {
    /// Takes the value out, leaving a default in its place until `give_back`.
    pub(crate) fn lend(&mut self) -> T {
        debug_assert!(!self.lent, "lent twice");
        self.lent = true;
        std::mem::take(&mut self.value)
    }

    pub(crate) fn give_back(&mut self, value: T) {
        self.value = value;
        self.lent = false;
    }
}

const LENT: &str = "the view state is lent to the views for this frame: use the `view` they were given";

impl<T> Deref for Lent<T> {
    type Target = T;
    fn deref(&self) -> &T {
        debug_assert!(!self.lent, "{LENT}");
        &self.value
    }
}

impl<T> DerefMut for Lent<T> {
    fn deref_mut(&mut self) -> &mut T {
        debug_assert!(!self.lent, "{LENT}");
        &mut self.value
    }
}

#[cfg(test)]
mod tests {
    use super::Lent;

    #[test]
    fn lends_and_takes_back() {
        let mut l = Lent::<Vec<i32>>::default();
        l.give_back(vec![1]);
        let v = l.lend();
        assert_eq!(v, [1]);
        l.give_back(vec![2]);
        assert_eq!(*l, [2]);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "lent to the views")]
    fn reading_while_lent_panics_in_debug() {
        let mut l = Lent::<Vec<i32>>::default();
        let _v = l.lend();
        let _ = l.len();
    }
}
