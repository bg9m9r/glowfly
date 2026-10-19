//! Which of a form's fields the user has edited, and whether a submit was tried: a field's
//! error or warning shows only after one of them (`ui::widgets::form_message::field`), so a
//! dialog doesn't open already complaining about a value nobody typed yet.

/// The fields of one form that were edited, by name, and whether a submit was attempted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Touched {
    fields: Vec<&'static str>,
    attempted: bool,
}

impl Touched {
    /// `field` was edited.
    pub fn mark(&mut self, field: &'static str) {
        if !self.fields.contains(&field) {
            self.fields.push(field);
        }
    }

    /// Whether `field`'s problem shows: it was edited, or a submit was attempted.
    pub fn shows(&self, field: &str) -> bool {
        self.attempted || self.fields.contains(&field)
    }

    /// A submit was attempted while the form was invalid: every field's problem shows.
    pub fn attempt(&mut self) {
        self.attempted = true;
    }

    /// Shows every problem: for a problem that comes from the form's own starting values, not
    /// from anything typed.
    pub fn all() -> Self {
        Self { fields: Vec::new(), attempted: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_shows_nothing() {
        let t = Touched::default();
        assert!(!t.shows("name") && !t.shows("url"));
    }

    #[test]
    fn mark_shows_only_that_field() {
        let mut t = Touched::default();
        t.mark("name");
        assert!(t.shows("name"));
        assert!(!t.shows("url"));
    }

    #[test]
    fn marking_twice_keeps_one_entry() {
        let mut t = Touched::default();
        t.mark("name");
        t.mark("name");
        assert_eq!(t.fields, vec!["name"]);
    }

    #[test]
    fn all_shows_every_field() {
        assert!(Touched::all().shows("folder"));
    }

    #[test]
    fn attempt_shows_every_field() {
        let mut t = Touched::default();
        t.attempt();
        assert!(t.shows("name") && t.shows("anything"));
    }
}
