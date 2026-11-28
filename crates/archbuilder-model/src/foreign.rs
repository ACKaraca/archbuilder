//! A foreign element: an element of an IFC import that has no native kind. Its shape is the mesh
//! stored in its record, in project coordinates and meters. Picking, the plan and the moves read
//! that mesh; a move encodes it again, so the stored shape always follows the element's identity.

use arc_base::Limits;
use arc_base::math::{Matrix4, Transform3};
use arc_doc::{GeometryBytes, Value};
use arc_mesh::{Quantization, TriMesh};

use crate::transform::Xform;
use crate::types::foreign_element;

/// The decoded mesh of a foreign element; `None` when the record has none or it does not decode.
#[must_use]
pub fn mesh(values: &[Option<Value>]) -> Option<TriMesh> {
    let Some(Some(Value::Geometry(bytes))) = values.get(foreign_element::MESH) else {
        return None;
    };
    TriMesh::decode(&bytes.0, &Limits::DEFAULT).ok()
}

/// The box around the mesh: its lowest and its highest corner, in meters.
#[must_use]
pub fn bounds(values: &[Option<Value>]) -> Option<([f64; 3], [f64; 3])> {
    let mesh = mesh(values)?;
    let mut points = mesh.positions().iter().map(|p| [p.x, p.y, p.z]);
    let first = points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| {
        (
            [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
            [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
        )
    }))
}

/// The corners of the plan box of the mesh, counterclockwise from the lowest corner.
#[must_use]
pub fn plan_outline(values: &[Option<Value>]) -> Option<Vec<[f64; 2]>> {
    let (lo, hi) = bounds(values)?;
    Some(vec![
        [lo[0], lo[1]],
        [hi[0], lo[1]],
        [hi[0], hi[1]],
        [lo[0], hi[1]],
    ])
}

/// The mesh field after `xf`, encoded again; `None` when the mesh does not decode, the transform
/// cannot apply (a mirror, see [`can_move`]) or the result is not finite.
#[must_use]
pub fn moved(values: &[Option<Value>], xf: &Xform) -> Option<Value> {
    let transform = transform3(xf)?;
    let moved = mesh(values)?.transformed(&transform).ok()?;
    let bytes = moved.encode(&Quantization::default()).ok()?;
    Some(Value::Geometry(GeometryBytes(bytes)))
}

/// Whether a foreign element can take `xf`. A mirror is refused: the imported shape keeps its
/// handedness, and a mirrored copy of a shape from another authoring tool is not a faithful one.
#[must_use]
pub const fn can_move(xf: &Xform) -> bool {
    !matches!(xf, Xform::Mirror { .. })
}

fn transform3(xf: &Xform) -> Option<Transform3> {
    let mut m = Matrix4::identity();
    match *xf {
        Xform::Translate([dx, dy]) => {
            m[(0, 3)] = dx;
            m[(1, 3)] = dy;
        }
        Xform::Rotate {
            base: [bx, by],
            angle,
        } => {
            let (s, c) = angle.sin_cos();
            m[(0, 0)] = c;
            m[(0, 1)] = -s;
            m[(1, 0)] = s;
            m[(1, 1)] = c;
            // The turn about the base: the base stays where it is.
            m[(0, 3)] = bx - (c * bx - s * by);
            m[(1, 3)] = by - (s * bx + c * by);
        }
        Xform::Mirror { .. } => return None,
    }
    Some(Transform3::from_matrix_unchecked(m))
}

/// The mesh field of a foreign element of the box `lo`..`hi`, for fixtures.
#[must_use]
pub fn box_value(lo: [f64; 3], hi: [f64; 3]) -> Option<Value> {
    let p = |i: usize| {
        arc_base::math::Point3::new(
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        )
    };
    let positions = (0..8).map(p).collect();
    // Outward faces: the bottom, top, front, back, left and right.
    let triangles = vec![
        [0, 2, 1],
        [1, 2, 3],
        [4, 5, 7],
        [4, 7, 6],
        [0, 1, 5],
        [0, 5, 4],
        [2, 6, 7],
        [2, 7, 3],
        [0, 4, 6],
        [0, 6, 2],
        [1, 3, 7],
        [1, 7, 5],
    ];
    let mesh = TriMesh::new(positions, triangles, &Limits::DEFAULT).ok()?;
    let bytes = mesh.encode(&Quantization::default()).ok()?;
    Some(Value::Geometry(GeometryBytes(bytes)))
}
