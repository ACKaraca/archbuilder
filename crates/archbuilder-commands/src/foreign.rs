//! Foreign elements (imported products that have no native kind) in the commands. A move rewrites
//! the stored mesh under the transform; a copy or a paste creates a new record with the moved mesh.
//! The copy has its own identifier, so the export gives it a new `GlobalId`. A mirror is refused
//! before anything changes (see [`archbuilder_model::foreign::can_move`]).

use arc_base::{Error, ObjectId, Result};
use arc_command::Context;
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::foreign;
use archbuilder_model::spatial;
use archbuilder_model::transform::Xform;
use archbuilder_model::types::{self, FOREIGN_ELEMENT};

use crate::error::codes;
use crate::structure::create;

/// The values of a foreign element after `xf`: its mesh moved, everything else as it is.
fn moved_values(values: &[Option<Value>], xf: &Xform) -> Result<Vec<Option<Value>>> {
    let mesh = foreign::moved(values, xf).ok_or_else(|| Error::new(codes::FOREIGN_SHAPE))?;
    let mut out = values.to_vec();
    let slot = out
        .get_mut(types::foreign_element::MESH)
        .ok_or_else(|| Error::new(codes::FOREIGN_SHAPE))?;
    *slot = Some(mesh);
    Ok(out)
}

/// Moves the foreign elements `ids` by `xf`: their mesh is written again.
pub(crate) fn move_foreign(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ids: &[ObjectId],
    xf: &Xform,
) -> Result<()> {
    for id in ids {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let mesh =
            foreign::moved(&object.values, xf).ok_or_else(|| Error::new(codes::FOREIGN_SHAPE))?;
        ctx.txn()?.modify(*id, FieldPath::new("mesh"), Some(mesh))?;
    }
    Ok(())
}

/// Copies the foreign elements `ids` moved by `xf`, onto `target` or their own storey.
pub(crate) fn copy_foreign(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ids: &[ObjectId],
    xf: &Xform,
    target: Option<ObjectId>,
) -> Result<Vec<ObjectId>> {
    let mut out = Vec::new();
    for id in ids {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let storey = target
            .or(object.header.owner)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let values = moved_values(&object.values, xf)?;
        out.push(create(ctx, FOREIGN_ELEMENT, Some(storey), values)?);
    }
    Ok(out)
}

/// Creates the pasted copy of a foreign element with the values of the clipboard, moved by `xf`.
pub(crate) fn paste_foreign(
    ctx: &mut Context<'_>,
    values: &[Option<Value>],
    xf: &Xform,
    storey: ObjectId,
) -> Result<ObjectId> {
    let values = moved_values(values, xf)?;
    create(ctx, FOREIGN_ELEMENT, Some(storey), values)
}
