//! Values views measure from a snapshot, kept in egui's temporary data between frames under a
//! key of what they were measured from (the snapshot's status stamp, a font, the zoom), so a frame
//! measures again only when that key changes.

use eframe::egui::{Context, Id};

/// The value kept under `id` for `key`, else `measure()`'s, which is then kept. The measurement
/// runs outside egui's data lock, so it may use the context (fonts, say).
pub(crate) fn kept<K, V>(ctx: &Context, id: Id, key: K, measure: impl FnOnce() -> V) -> V
where
    K: Clone + PartialEq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    if let Some((k, v)) = ctx.data(|d| d.get_temp::<(K, V)>(id))
        && k == key
    {
        return v;
    }
    let v = measure();
    ctx.data_mut(|d| d.insert_temp(id, (key, v.clone())));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_measures_once_per_key() {
        let ctx = Context::default();
        let id = Id::new("memo");
        let calls = std::cell::Cell::new(0);
        let get = |key: u64, id: Id| {
            kept(&ctx, id, key, || {
                calls.set(calls.get() + 1);
                5
            })
        };
        assert_eq!(get(1, id), 5);
        assert_eq!(get(1, id), 5);
        assert_eq!(calls.get(), 1, "the same key measures once");
        get(2, id);
        assert_eq!(calls.get(), 2, "a new key measures again");
        get(2, Id::new("other"));
        assert_eq!(calls.get(), 3, "a different id shares nothing");
    }
}
