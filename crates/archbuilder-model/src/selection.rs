//! Selection sets: named lists of elements saved in the document. A set stores element
//! identifiers, which never change; a deleted element leaves the set when the element is deleted
//! (the reference clears). Rule-based sets are not part of this record.

use std::sync::Arc;

use arc_base::ObjectId;
use arc_doc::{Object, Snapshot, Value};

use crate::spatial;
use crate::types::{MAX_SET_MEMBERS, SELECTION_SET, selection_set};

/// The members of a set record, in stored order, without duplicates.
#[must_use]
pub fn members(values: &[Option<Value>]) -> Vec<ObjectId> {
    let mut out: Vec<ObjectId> = Vec::new();
    if let Some(Some(Value::List(items))) = values.get(selection_set::MEMBERS) {
        for item in items.iter().take(MAX_SET_MEMBERS) {
            if let Value::Record(fields) = item
                && let Some(Some(Value::Reference(id))) = fields.get(selection_set::MEMBER_OBJECT)
                && !out.contains(id)
            {
                out.push(*id);
            }
        }
    }
    out
}

/// The value of the `members` field for `ids`; absent when empty.
#[must_use]
pub fn members_value(ids: &[ObjectId]) -> Option<Value> {
    (!ids.is_empty()).then(|| {
        Value::List(
            ids.iter()
                .take(MAX_SET_MEMBERS)
                .map(|id| Value::Record(vec![Some(Value::Reference(*id))]))
                .collect(),
        )
    })
}

/// The values of a new set record.
#[must_use]
pub fn values(name: &str, ids: &[ObjectId]) -> Vec<Option<Value>> {
    vec![Some(Value::String(name.to_owned())), members_value(ids)]
}

/// Every selection set, by name.
#[must_use]
pub fn all(snapshot: &Snapshot) -> Vec<Arc<Object>> {
    let mut found = spatial::objects(snapshot, SELECTION_SET);
    found.sort_by(|a, b| {
        crate::types::text(&a.values, selection_set::NAME)
            .cmp(&crate::types::text(&b.values, selection_set::NAME))
            .then(a.id.cmp(&b.id))
    });
    found
}

#[cfg(test)]
mod tests {
    use arc_base::id::Identifier;

    use super::*;

    #[test]
    fn members_round_trip_and_repeat_once() {
        let a = ObjectId::from_u128(1);
        let b = ObjectId::from_u128(2);
        let mut v = values("Fire doors", &[a, b]);
        assert_eq!(members(&v), vec![a, b]);
        v[1] = members_value(&[a, a]);
        assert_eq!(members(&v), vec![a]);
        assert!(values("x", &[])[1].is_none());
    }
}
