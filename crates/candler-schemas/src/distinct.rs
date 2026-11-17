//! The one check that a list holds no name twice.
//!
//! Every refusal that says "this was declared twice" finds its pair here, so every one of them
//! follows the same rule: **the report names the first clash in input order**, the first item that
//! repeats an item before it. The earlier item is the one that stood first; the later one is the
//! repeat. A caller that names one side names the repeat. The caller keeps its own error, code
//! and side; this module owns only the finding.
//!
//! Both functions read the items once, in order, and stop at the first clash.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

/// The first item of `items` equal to one before it, or `None` if every item is distinct.
///
/// "First" is by position in `items`, not by value: of `[b, a, a, b]` it is the second `a`.
#[must_use = "the repeat is the only result"]
pub fn first_repeat<T: Ord>(items: impl IntoIterator<Item = T>) -> Option<T> {
    let mut seen: BTreeSet<T> = BTreeSet::new();
    for item in items {
        if seen.contains(&item) {
            return Some(item);
        }
        seen.insert(item);
    }
    None
}

/// The first two items of `items` whose `key` is equal, as `(earlier, later)`, or `None` if the
/// keys are all distinct. `later` is the first item whose key an earlier item has; `earlier` is the
/// first item that had it.
///
/// Items that are equal under `key` but differ otherwise, such as `A` and `a` under a case fold,
/// are a clash; this is what a caller that names both sides needs.
#[must_use = "the pair is the only result"]
pub fn first_clash<T, K: Ord>(
    items: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> K,
) -> Option<(T, T)> {
    let mut seen: BTreeMap<K, T> = BTreeMap::new();
    for item in items {
        match seen.entry(key(&item)) {
            Entry::Occupied(held) => return Some((held.remove(), item)),
            // EXPLICIT: a new key is remembered with the item that first had it
            Entry::Vacant(slot) => {
                slot.insert(item);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{first_clash, first_repeat};

    type Item = (&'static str, u8);
    type Row = (&'static str, &'static [Item], Option<(Item, Item)>);

    /// Guards: the rule every duplicate refusal shares: the first clash in input order is the one
    /// reported, whatever the values sort as. Oracle: hand-written rows, each worked by reading
    /// the list left to right.
    #[test]
    fn the_first_clash_in_input_order_is_reported() {
        // (name, items, the repeat)
        let rows: [(&str, &[&str], Option<&str>); 7] = [
            ("empty", &[], None),
            ("one item", &["a"], None),
            ("all distinct", &["c", "a", "b"], None),
            ("a pair", &["a", "b", "a"], Some("a")),
            // `b` repeats first by position; the sorted minimum is `a`
            (
                "two repeats, larger first",
                &["b", "b", "a", "a"],
                Some("b"),
            ),
            (
                "the later pair wins by position",
                &["a", "b", "c", "c", "a"],
                Some("c"),
            ),
            ("three of one", &["x", "x", "x"], Some("x")),
        ];
        for (name, items, want) in rows {
            assert_eq!(first_repeat(items.iter().copied()), want, "{name}");
        }
    }

    /// Guards: `first_clash` returns the earlier and the later item, and clashes by key, not by
    /// item. Oracle: hand-written rows of `(label, key)` items.
    #[test]
    fn a_clash_by_key_returns_both_items() {
        let by_key = |item: &Item| item.1;
        // (name, items, the pair)
        let rows: [Row; 4] = [
            ("no clash", &[("a", 1), ("b", 2)], None),
            (
                "same key, other label",
                &[("A", 1), ("a", 1)],
                Some((("A", 1), ("a", 1))),
            ),
            (
                "the earlier is the first that held the key",
                &[("p", 1), ("q", 2), ("r", 1), ("s", 1)],
                Some((("p", 1), ("r", 1))),
            ),
            (
                "a clash found before a larger one",
                &[("a", 9), ("b", 9), ("c", 1), ("d", 1)],
                Some((("a", 9), ("b", 9))),
            ),
        ];
        for (name, items, want) in rows {
            assert_eq!(first_clash(items.iter().copied(), by_key), want, "{name}");
        }
    }
}
