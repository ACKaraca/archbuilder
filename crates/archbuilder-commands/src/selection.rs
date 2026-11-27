//! The commands of selection sets (named, saved lists of elements): `archbuilder.set.create`,
//! `archbuilder.set.update` (add and remove members), `archbuilder.set.rename`,
//! `archbuilder.set.delete` and `archbuilder.set.select` (the members become, or join, the
//! selection; no transaction). Each change is one transaction, so one undo step.
//!
//! A set stores element identifiers; deleting an element removes it from every set.

use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{FieldPath, Snapshot};
use archbuilder_model::selection;
use archbuilder_model::spatial;
use archbuilder_model::types::{self, SELECTION_SET};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{SelectMode, SelectObjects, create, ensure_valid, expect, select_objects};

/// A message of the catalog, named by identifier.
struct MessageId {
    message_id: &'static str,
}

/// The identifier of the create-set command.
pub const SET_CREATE: &str = "archbuilder.set.create";
/// The identifier of the update-set command.
pub const SET_UPDATE: &str = "archbuilder.set.update";
/// The identifier of the rename-set command.
pub const SET_RENAME: &str = "archbuilder.set.rename";
/// The identifier of the delete-set command.
pub const SET_DELETE: &str = "archbuilder.set.delete";
/// The identifier of the select-set command.
pub const SET_SELECT: &str = "archbuilder.set.select";

/// Parameters of `archbuilder.set.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateSet {
    /// The name; unused by the other sets.
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
    /// The members; without it, the elements of the selection.
    #[serde(default)]
    #[schemars(length(max = 100000))]
    pub objects: Option<Vec<IdParam>>,
}

/// What `archbuilder.set.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SetMade {
    /// The new set.
    pub set: IdParam,
    /// How many members it holds.
    pub members: usize,
}

/// Parameters of `archbuilder.set.update`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateSet {
    /// The set.
    pub set: IdParam,
    /// Elements to add.
    #[serde(default)]
    #[schemars(length(max = 100000))]
    pub add: Vec<IdParam>,
    /// Members to remove.
    #[serde(default)]
    #[schemars(length(max = 100000))]
    pub remove: Vec<IdParam>,
}

/// Parameters of `archbuilder.set.rename`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameSet {
    /// The set.
    pub set: IdParam,
    /// The new name.
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
}

/// Parameters of `archbuilder.set.delete`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteSet {
    /// The set.
    pub set: IdParam,
}

/// Parameters of `archbuilder.set.select`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectSet {
    /// The set.
    pub set: IdParam,
    /// The combination with the selection; the members replace it by default.
    #[serde(default)]
    pub mode: SelectMode,
}

/// The elements of `ids`, in order and without repeats; any other object is refused.
fn elements(snapshot: &Snapshot, ids: impl IntoIterator<Item = ObjectId>) -> Result<Vec<ObjectId>> {
    let mut out: Vec<ObjectId> = Vec::new();
    for id in ids {
        let element = spatial::type_of(snapshot, id).is_some_and(|k| types::is_element(&k));
        if !element {
            return Err(Error::new(codes::KIND_MISMATCH));
        }
        if !out.contains(&id) {
            out.push(id);
        }
    }
    if out.len() > types::MAX_SET_MEMBERS {
        return Err(Error::new(codes::KIND_MISMATCH));
    }
    Ok(out)
}

fn members_of(ctx: &Context<'_>, set: ObjectId) -> Result<Vec<ObjectId>> {
    let object = expect(ctx.snapshot(), set, SELECTION_SET)?;
    Ok(selection::members(&object.values))
}

fn write_members(ctx: &mut Context<'_>, set: ObjectId, members: &[ObjectId]) -> Result<()> {
    ctx.txn()?.modify(
        set,
        FieldPath::new("members"),
        selection::members_value(members),
    )?;
    ensure_valid(ctx)
}

fn make(ctx: &mut Context<'_>, p: CreateSet) -> Result<SetMade> {
    let project =
        spatial::project(ctx.snapshot()).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let chosen = match p.objects {
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        // The selection may hold levels and the like; only its elements are members.
        None => ctx
            .selection()
            .iter()
            .copied()
            .filter(|id| {
                spatial::type_of(ctx.snapshot(), *id).is_some_and(|k| types::is_element(&k))
            })
            .collect::<Vec<_>>(),
    };
    let members = elements(ctx.snapshot(), chosen)?;
    let id = create(
        ctx,
        SELECTION_SET,
        Some(project.id),
        selection::values(p.name.trim(), &members),
    )?;
    ensure_valid(ctx)?;
    Ok(SetMade {
        set: IdParam(id),
        members: members.len(),
    })
}

fn update(ctx: &mut Context<'_>, p: UpdateSet) -> Result<SetMade> {
    let mut members = members_of(ctx, p.set.0)?;
    let add = elements(ctx.snapshot(), p.add.into_iter().map(|IdParam(id)| id))?;
    members.retain(|m| !p.remove.iter().any(|IdParam(r)| r == m));
    for id in add {
        if !members.contains(&id) {
            members.push(id);
        }
    }
    if members.len() > types::MAX_SET_MEMBERS {
        return Err(Error::new(codes::KIND_MISMATCH));
    }
    write_members(ctx, p.set.0, &members)?;
    Ok(SetMade {
        set: p.set,
        members: members.len(),
    })
}

fn rename(ctx: &mut Context<'_>, p: RenameSet) -> Result<()> {
    expect(ctx.snapshot(), p.set.0, SELECTION_SET)?;
    ctx.txn()?.modify(
        p.set.0,
        FieldPath::new("name"),
        Some(arc_doc::Value::String(p.name.trim().to_owned())),
    )?;
    ensure_valid(ctx)
}

fn delete(ctx: &mut Context<'_>, p: DeleteSet) -> Result<()> {
    expect(ctx.snapshot(), p.set.0, SELECTION_SET)?;
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    txn.delete(p.set.0)?;
    txn.apply_delete_rules(&limits)?;
    ensure_valid(ctx)
}

fn select(ctx: &mut Context<'_>, p: SelectSet) -> Result<()> {
    let members = members_of(ctx, p.set.0)?;
    select_objects(
        ctx,
        SelectObjects {
            objects: members.into_iter().map(IdParam).collect(),
            mode: p.mode,
            gesture: None,
        },
    )
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(CommandSpec::new(
        SET_CREATE,
        MessageId {
            message_id: "ab-cmd-set-create-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-set-create-description",
        }
        .message_id,
        make,
    ))?;
    registry.register(CommandSpec::new(
        SET_UPDATE,
        MessageId {
            message_id: "ab-cmd-set-update-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-set-update-description",
        }
        .message_id,
        update,
    ))?;
    registry.register(CommandSpec::new(
        SET_RENAME,
        MessageId {
            message_id: "ab-cmd-set-rename-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-set-rename-description",
        }
        .message_id,
        rename,
    ))?;
    registry.register(CommandSpec::new(
        SET_DELETE,
        MessageId {
            message_id: "ab-cmd-set-delete-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-set-delete-description",
        }
        .message_id,
        delete,
    ))?;
    registry.register(
        CommandSpec::new(
            SET_SELECT,
            MessageId {
                message_id: "ab-cmd-set-select-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-set-select-description",
            }
            .message_id,
            select,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read]),
    )?;
    Ok(())
}
