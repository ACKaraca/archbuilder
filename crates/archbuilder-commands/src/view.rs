//! The commands of plan views: `archbuilder.view.duplicate`, `archbuilder.view.rename`,
//! `archbuilder.view.delete` (the last view of a storey stays), `archbuilder.view.hide_elements`
//! and `archbuilder.view.show_elements` (the elements hidden in one view), and
//! `archbuilder.view.ensure_defaults`, which gives a storey without a view its default one, and
//! `archbuilder.view.set`, which sets one setting of a view.
//!
//! Each is one transaction, so one undo step. Settings such as the cut height, the scale and the
//! category flags are edited with `archbuilder.property.set` or `archbuilder.view.set`. A storey
//! without a view (a document saved before plan views) shows a default view the document does not
//! hold: duplicate, rename, hide and set take the storey for it and make the view in their own
//! transaction first.

use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, IdParam, Registry};
use arc_doc::{FieldPath, Value};
use archbuilder_model::spatial;
use archbuilder_model::types::{PLAN_VIEW, STOREY};
use archbuilder_model::view::{self, PlanView};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{create, ensure_valid, expect};

/// The identifier of the duplicate-view command.
pub const VIEW_DUPLICATE: &str = "archbuilder.view.duplicate";
/// The identifier of the rename-view command.
pub const VIEW_RENAME: &str = "archbuilder.view.rename";
/// The identifier of the delete-view command.
pub const VIEW_DELETE: &str = "archbuilder.view.delete";
/// The identifier of the hide-in-view command.
pub const VIEW_HIDE: &str = "archbuilder.view.hide_elements";
/// The identifier of the show-in-view command.
pub const VIEW_SHOW: &str = "archbuilder.view.show_elements";
/// The identifier of the command that gives storeys their default views.
pub const VIEW_ENSURE_DEFAULTS: &str = "archbuilder.view.ensure_defaults";
/// The identifier of the command that sets one setting of a view.
pub const VIEW_SET: &str = "archbuilder.view.set";

/// The view record `id` names: a plan view itself, or for a storey its first view. A storey without
/// a view (a document saved before plan views) shows a default view that the document does not hold;
/// it is made here, in the command's transaction, so the change that needed it and its making are
/// one undo step.
pub(crate) fn real_view(ctx: &mut Context<'_>, id: ObjectId) -> Result<ObjectId> {
    match spatial::type_of(ctx.snapshot(), id).as_deref() {
        Some(PLAN_VIEW) => Ok(id),
        Some(STOREY) => {
            if let Some(first) = view::of_storey(ctx.snapshot(), id).first() {
                return Ok(first.id);
            }
            let name = spatial::get(ctx.snapshot(), id)
                .and_then(|s| {
                    archbuilder_model::types::text(
                        &s.values,
                        archbuilder_model::types::storey::NAME,
                    )
                    .map(str::to_owned)
                })
                .unwrap_or_default();
            create_default(ctx, id, &name)
        }
        _ => Err(Error::new(codes::KIND_MISMATCH)),
    }
}

/// Creates the default plan view of `storey`, named `name`.
pub(crate) fn create_default(
    ctx: &mut Context<'_>,
    storey: ObjectId,
    name: &str,
) -> Result<ObjectId> {
    create(ctx, PLAN_VIEW, Some(storey), PlanView::new(name).values())
}

/// Parameters of `archbuilder.view.duplicate`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DuplicateView {
    /// The view.
    pub view: IdParam,
    /// The name of the copy; else the first free `<name> <n>`.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    pub name: Option<String>,
}

/// What `archbuilder.view.duplicate` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ViewMade {
    /// The new view.
    pub view: IdParam,
}

/// Parameters of `archbuilder.view.rename`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameView {
    /// The view.
    pub view: IdParam,
    /// The new name.
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
}

/// Parameters of `archbuilder.view.delete`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteView {
    /// The view.
    pub view: IdParam,
}

/// Parameters of `archbuilder.view.hide_elements` and `archbuilder.view.show_elements`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewElements {
    /// The view.
    pub view: IdParam,
    /// The elements; for `show_elements` none shows every hidden element again.
    #[serde(default)]
    #[schemars(length(max = 100000))]
    pub objects: Vec<IdParam>,
}

/// What `archbuilder.view.ensure_defaults` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DefaultsMade {
    /// The views made.
    pub made: usize,
}

fn duplicate(ctx: &mut Context<'_>, p: DuplicateView) -> Result<ViewMade> {
    let source = real_view(ctx, p.view.0)?;
    let source = expect(ctx.snapshot(), source, PLAN_VIEW)?;
    let settings =
        PlanView::from_values(&source.values).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let siblings = source
        .header
        .owner
        .map(|s| view::of_storey(ctx.snapshot(), s))
        .unwrap_or_default();
    let taken = |name: &str| {
        siblings.iter().any(|v| {
            archbuilder_model::types::text(&v.values, 0)
                .is_some_and(|n| archbuilder_model::names::same(n, name))
        })
    };
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => (2..)
            .map(|n| format!("{} {n}", settings.name))
            .find(|n| !taken(n))
            .unwrap_or_default(),
    };
    let copy = PlanView { name, ..settings };
    let id = create(ctx, PLAN_VIEW, source.header.owner, copy.values())?;
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    Ok(ViewMade { view: IdParam(id) })
}

fn rename(ctx: &mut Context<'_>, p: RenameView) -> Result<()> {
    let id = real_view(ctx, p.view.0)?;
    ctx.txn()?.modify(
        id,
        FieldPath::new("name"),
        Some(Value::String(p.name.trim().to_owned())),
    )?;
    ensure_valid(ctx)
}

fn delete(ctx: &mut Context<'_>, p: DeleteView) -> Result<()> {
    let object = expect(ctx.snapshot(), p.view.0, PLAN_VIEW)?;
    let alone = object
        .header
        .owner
        .is_none_or(|s| view::of_storey(ctx.snapshot(), s).len() <= 1);
    if alone {
        return Err(Error::new(codes::LAST_VIEW));
    }
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    txn.delete(p.view.0)?;
    txn.apply_delete_rules(&limits)?;
    ensure_valid(ctx)
}

fn set_hidden(ctx: &mut Context<'_>, id: ObjectId, hidden: &[ObjectId]) -> Result<()> {
    let value = (!hidden.is_empty()).then(|| view::hidden_value(hidden));
    ctx.txn()?.modify(id, FieldPath::new("hidden"), value)?;
    ensure_valid(ctx)
}

fn hide(ctx: &mut Context<'_>, p: ViewElements) -> Result<()> {
    let view = real_view(ctx, p.view.0)?;
    let object = expect(ctx.snapshot(), view, PLAN_VIEW)?;
    let mut hidden = PlanView::from_values(&object.values)
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
        .hidden;
    for IdParam(id) in p.objects {
        let key =
            spatial::type_of(ctx.snapshot(), id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        if !archbuilder_model::types::is_element(&key) {
            return Err(Error::new(codes::KIND_MISMATCH));
        }
        if !hidden.contains(&id) {
            hidden.push(id);
        }
    }
    hidden.truncate(archbuilder_model::types::MAX_HIDDEN);
    set_hidden(ctx, view, &hidden)
}

/// Parameters of `archbuilder.view.set`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetView {
    /// The view, or a storey for its default view (made by this command when the storey has no
    /// view).
    pub view: IdParam,
    /// The setting: a field of the view record, as `archbuilder.property.set` takes it.
    #[schemars(length(min = 1, max = 64))]
    pub field: String,
    /// The value, as `archbuilder.property.set` takes it.
    pub value: serde_json::Value,
}

fn set(ctx: &mut Context<'_>, p: SetView) -> Result<ViewMade> {
    let id = real_view(ctx, p.view.0)?;
    crate::structure::set_property(
        ctx,
        crate::structure::SetProperty {
            objects: vec![IdParam(id)],
            field: p.field,
            value: p.value,
        },
    )?;
    ensure_valid(ctx)?;
    Ok(ViewMade { view: IdParam(id) })
}

fn show(ctx: &mut Context<'_>, p: ViewElements) -> Result<()> {
    let object = expect(ctx.snapshot(), p.view.0, PLAN_VIEW)?;
    let mut hidden = PlanView::from_values(&object.values)
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
        .hidden;
    if p.objects.is_empty() {
        hidden.clear();
    } else {
        hidden.retain(|h| !p.objects.iter().any(|IdParam(id)| id == h));
    }
    set_hidden(ctx, p.view.0, &hidden)
}

/// Parameters of `archbuilder.view.ensure_defaults`: none.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

fn ensure_defaults(ctx: &mut Context<'_>, _: NoParams) -> Result<DefaultsMade> {
    let missing: Vec<(ObjectId, String)> = spatial::objects(ctx.snapshot(), STOREY)
        .iter()
        .filter(|s| view::of_storey(ctx.snapshot(), s.id).is_empty())
        .map(|s| {
            let name = archbuilder_model::types::text(&s.values, 0).unwrap_or_default();
            (s.id, name.to_owned())
        })
        .collect();
    for (storey, name) in &missing {
        create_default(ctx, *storey, name)?;
    }
    if !missing.is_empty() {
        ensure_valid(ctx)?;
    }
    Ok(DefaultsMade {
        made: missing.len(),
    })
}

/// A message of the catalog, named by identifier.
struct Message {
    message_id: &'static str,
}

fn spec<P, R, F>(id: &str, label: Message, description: Message, run: F) -> CommandSpec
where
    P: serde::de::DeserializeOwned + JsonSchema,
    R: Serialize + JsonSchema,
    F: Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static,
{
    CommandSpec::new(id, label.message_id, description.message_id, run)
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    for s in [
        spec(
            VIEW_DUPLICATE,
            Message {
                message_id: "ab-cmd-view-duplicate-label",
            },
            Message {
                message_id: "ab-cmd-view-duplicate-description",
            },
            duplicate,
        ),
        spec(
            VIEW_RENAME,
            Message {
                message_id: "ab-cmd-view-rename-label",
            },
            Message {
                message_id: "ab-cmd-view-rename-description",
            },
            rename,
        ),
        spec(
            VIEW_DELETE,
            Message {
                message_id: "ab-cmd-view-delete-label",
            },
            Message {
                message_id: "ab-cmd-view-delete-description",
            },
            delete,
        ),
        spec(
            VIEW_HIDE,
            Message {
                message_id: "ab-cmd-view-hide-label",
            },
            Message {
                message_id: "ab-cmd-view-hide-description",
            },
            hide,
        ),
        spec(
            VIEW_SHOW,
            Message {
                message_id: "ab-cmd-view-show-label",
            },
            Message {
                message_id: "ab-cmd-view-show-description",
            },
            show,
        ),
        spec(
            VIEW_SET,
            Message {
                message_id: "ab-cmd-view-set-label",
            },
            Message {
                message_id: "ab-cmd-view-set-description",
            },
            set,
        ),
        spec(
            VIEW_ENSURE_DEFAULTS,
            Message {
                message_id: "ab-cmd-view-defaults-label",
            },
            Message {
                message_id: "ab-cmd-view-defaults-description",
            },
            ensure_defaults,
        ),
    ] {
        registry.register(s)?;
    }
    Ok(())
}
