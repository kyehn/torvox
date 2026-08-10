//! Procedural geometry generator for render-verification scenes.
//!
//! Ported from wgpu-example `src/raytracing.rs:214-429` (`add_floor`,
//! `add_sphere`, `add_box`, `add_torus`, `rotate_y` + `SceneData`), which the
//! reference audit flagged as the zero-dependency, unit-testable geometry
//! source for torvox's off-screen render verification path
//! (research-wgpu-example.md §6.2).
//!
//! This module intentionally has NO GPU resources: it only produces CPU-side
//! vertex/normal/index lists plus hand-written matrix helpers (torvox has no
//! glam/nalgebra dependency; see `context.rs::orthographic_projection` for the
//! same hand-rolled style). Depth texture creation lives here too so the
//! off-screen grid verification path stays self-contained and never touches
//! the `Renderer` struct (main render path keeps zero depth attachments).
//!
//! `#[cfg(test)]`: crate-test-only (see `render/mod.rs`).
//!
//! # Requirements
//! - FR-056 — off-screen render-verification path (procedural geometry + depth-attached LOD grid) without production depth attachments

/// CPU-side triangle mesh: positions, per-vertex normals, triangle indices.
#[derive(Debug, Clone, Default)]
pub struct SceneData {
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl SceneData {
    /// Append another mesh's geometry to this one, re-basing indices.
    pub fn append(&mut self, other: &SceneData) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.normals.extend_from_slice(&other.normals);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    /// Axis-aligned bounding box as `(min, max)` over all vertices.
    pub fn bounding_box(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for v in &self.vertices {
            for axis in 0..3 {
                min[axis] = min[axis].min(v[axis]);
                max[axis] = max[axis].max(v[axis]);
            }
        }
        if self.vertices.is_empty() {
            None
        } else {
            Some((min, max))
        }
    }

    /// Rotate every vertex around the Y axis by `angle` radians.
    pub fn rotate_y(&mut self, angle: f32) {
        let (sin, cos) = angle.sin_cos();
        for v in &mut self.vertices {
            let (x, z) = (v[0], v[2]);
            v[0] = cos * x + sin * z;
            v[2] = -sin * x + cos * z;
        }
        for n in &mut self.normals {
            let (x, z) = (n[0], n[2]);
            n[0] = cos * x + sin * z;
            n[2] = -sin * x + cos * z;
        }
    }
}

/// Push a single quad (4 vertices + 2 triangles) with one shared normal.
fn push_quad(scene: &mut SceneData, corners: [[f32; 3]; 4], normal: [f32; 3]) {
    let base = scene.vertices.len() as u32;
    for corner in corners {
        scene.vertices.push(corner);
        scene.normals.push(normal);
    }
    scene
        .indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Ground plane quad centered at `(0, y, 0)` spanning `size` on X and Z.
pub fn add_floor(scene: &mut SceneData, size: f32, y: f32) {
    let half = size * 0.5;
    let corners = [
        [-half, y, -half],
        [half, y, -half],
        [half, y, half],
        [-half, y, half],
    ];
    push_quad(scene, corners, [0.0, 1.0, 0.0]);
}

/// UV sphere with `stacks` latitude bands and `slices` longitude segments
/// (wgpu-example uses 24 × 48).
pub fn add_sphere(scene: &mut SceneData, center: [f32; 3], radius: f32, stacks: u32, slices: u32) {
    let mut previous_ring = Vec::with_capacity((slices + 1) as usize);
    for stack in 0..=stacks {
        let phi = std::f32::consts::PI * (stack as f32 / stacks as f32);
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        let mut ring = Vec::with_capacity((slices + 1) as usize);
        for slice in 0..=slices {
            let theta = 2.0 * std::f32::consts::PI * (slice as f32 / slices as f32);
            let normal = [sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin()];
            scene.vertices.push([
                center[0] + radius * normal[0],
                center[1] + radius * normal[1],
                center[2] + radius * normal[2],
            ]);
            scene.normals.push(normal);
            ring.push((scene.vertices.len() - 1) as u32);
        }
        if stack > 0 {
            for slice in 0..slices {
                let a = previous_ring[slice as usize];
                let b = previous_ring[(slice + 1) as usize];
                let c = ring[(slice + 1) as usize];
                let d = ring[slice as usize];
                scene.indices.extend_from_slice(&[a, b, c, a, c, d]);
            }
        }
        previous_ring = ring;
    }
}

/// Axis-aligned box centered at `center` with half-extents `half`, rotated
/// around Y by `yaw` radians. Each face is built by fixing the coordinate on
/// the normal axis and sweeping ±half on the other two axes, then rotating
/// (raytracing.rs:356-370).
pub fn add_box(scene: &mut SceneData, center: [f32; 3], half: [f32; 3], yaw: f32) {
    let (sin, cos) = yaw.sin_cos();
    let rotate =
        |p: [f32; 3]| -> [f32; 3] { [cos * p[0] + sin * p[2], p[1], -sin * p[0] + cos * p[2]] };
    // Face normal plus the axis index it is aligned with.
    let faces: [([f32; 3], usize); 6] = [
        ([1.0, 0.0, 0.0], 0),
        ([-1.0, 0.0, 0.0], 0),
        ([0.0, 1.0, 0.0], 1),
        ([0.0, -1.0, 0.0], 1),
        ([0.0, 0.0, 1.0], 2),
        ([0.0, 0.0, -1.0], 2),
    ];
    for (normal, axis) in faces {
        let fixed = normal[axis] * half[axis];
        let other: Vec<usize> = (0..3).filter(|&i| i != axis).collect();
        let (a, b) = (other[0], other[1]);
        // Corner order is CCW when viewed from outside the +axis face;
        // cull_mode is None on the grid/verification path so winding is
        // cosmetic, but keep it consistent for the -axis faces too.
        let mut corners = [[0.0f32; 3]; 4];
        for (i, corner) in corners.iter_mut().enumerate() {
            corner[axis] = fixed;
            corner[a] = if i & 1 == 0 { half[a] } else { -half[a] };
            corner[b] = if i & 2 == 0 { -half[b] } else { half[b] };
        }
        let rotated: Vec<[f32; 3]> = corners
            .iter()
            .map(|&c| {
                let r = rotate(c);
                [center[0] + r[0], center[1] + r[1], center[2] + r[2]]
            })
            .collect();
        push_quad(scene, rotated.try_into().expect("four corners"), normal);
    }
}

/// Torus in the XZ plane centered at `center`: `major` radius to the tube
/// center, `minor` tube radius, `segments` around the main circle,
/// `rings` around the tube.
pub fn add_torus(
    scene: &mut SceneData,
    center: [f32; 3],
    major: f32,
    minor: f32,
    segments: u32,
    rings: u32,
) {
    let mut previous_ring: Vec<u32> = Vec::new();
    for segment in 0..=segments {
        let theta = 2.0 * std::f32::consts::PI * (segment as f32 / segments as f32);
        let (sin_t, cos_t) = theta.sin_cos();
        let mut ring = Vec::with_capacity((rings + 1) as usize);
        for ring_idx in 0..=rings {
            let phi = 2.0 * std::f32::consts::PI * (ring_idx as f32 / rings as f32);
            let (sin_p, cos_p) = phi.sin_cos();
            // Tube point relative to the torus center.
            let tube_x = (major + minor * cos_p) * cos_t;
            let tube_y = minor * sin_p;
            let tube_z = (major + minor * cos_p) * sin_t;
            // Normal points from the tube center toward the surface.
            let normal = [cos_p * cos_t, sin_p, cos_p * sin_t];
            scene
                .vertices
                .push([center[0] + tube_x, center[1] + tube_y, center[2] + tube_z]);
            scene.normals.push(normal);
            ring.push((scene.vertices.len() - 1) as u32);
        }
        if segment > 0 {
            for ring_idx in 0..rings {
                let a = previous_ring[ring_idx as usize];
                let b = previous_ring[(ring_idx + 1) as usize];
                let c = ring[(ring_idx + 1) as usize];
                let d = ring[ring_idx as usize];
                scene.indices.extend_from_slice(&[a, b, c, a, c, d]);
            }
        }
        previous_ring = ring;
    }
}

// ── Hand-written matrix helpers ──────────────────────────────────────────
// Column-major `[[f32; 4]; 4]` (each inner array is one COLUMN), matching
// how `GpuUniforms.projection` is consumed by WGSL `mat4x4` (pipeline.rs:34,
// cell.wgsl:1-8).

/// Multiply two column-major 4x4 matrices: `a * b`.
pub fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            result[col][row] = (0..4).map(|k| a[k][row] * b[col][k]).sum();
        }
    }
    result
}

/// Right-handed perspective projection mapping view-space `z ∈ [-near, -far]`
/// to `ndc z ∈ [0, 1]` (Vulkan convention), column-major.
///
/// Derivation: `clip_z = a*z + b`, `clip_w = -z` with `a = far/(near-far)`,
/// `b = far*near/(near-far)` (both negative for `near < far`).
pub fn perspective(fovy_radians: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fovy_radians * 0.5).tan();
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, (far * near) / (near - far), 0.0],
    ]
}

/// Right-handed look-at view matrix, column-major (ported from wgpu-example
/// `look_at_rh`, raytracing.rs:70-92).
pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let z_axis = normalize3(sub3(eye, target));
    let x_axis = normalize3(cross3(up, z_axis));
    let y_axis = cross3(z_axis, x_axis);
    [
        [x_axis[0], y_axis[0], z_axis[0], 0.0],
        [x_axis[1], y_axis[1], z_axis[1], 0.0],
        [x_axis[2], y_axis[2], z_axis[2], 0.0],
        [
            -dot3(x_axis, eye),
            -dot3(y_axis, eye),
            -dot3(z_axis, eye),
            1.0,
        ],
    ]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = dot3(v, v).sqrt();
    if len > 0.0 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        v
    }
}

/// Create a `Depth32Float` render-attachment texture view for the off-screen
/// grid verification path. Deliberately a free function: the main `Renderer`
/// keeps zero depth attachments (2D terminal rendering needs none).
pub fn create_depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Grid Depth Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    #[test]
    fn floor_has_four_vertices_and_two_triangles() {
        let mut scene = SceneData::default();
        add_floor(&mut scene, 10.0, 0.0);
        assert_eq!(scene.vertices.len(), 4);
        assert_eq!(scene.normals.len(), 4);
        assert_eq!(scene.indices.len(), 6);
        assert_eq!(scene.normals[0], [0.0, 1.0, 0.0]);
        assert_eq!(scene.bounding_box().unwrap().0, [-5.0, 0.0, -5.0]);
        assert_eq!(scene.bounding_box().unwrap().1, [5.0, 0.0, 5.0]);
    }

    #[test]
    fn sphere_has_expected_counts_and_unit_normals() {
        let mut scene = SceneData::default();
        add_sphere(&mut scene, [0.0, 0.0, 0.0], 1.0, 24, 48);
        assert_eq!(scene.vertices.len(), (24 + 1) * (48 + 1));
        assert_eq!(scene.vertices.len(), scene.normals.len());
        // 24 stacks -> 24 bands of 48 quads -> 24*48*6 indices.
        assert_eq!(scene.indices.len(), 24 * 48 * 6);
        for n in &scene.normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < EPS, "normal not unit: {n:?}");
        }
        // Sphere surface points sit at radius 1 from center.
        for v in &scene.vertices {
            let r = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!((r - 1.0).abs() < EPS, "vertex off sphere: {v:?} r={r}");
        }
    }

    #[test]
    fn box_has_six_faces_and_bounded_extent() {
        let mut scene = SceneData::default();
        add_box(&mut scene, [0.0, 0.0, 0.0], [1.0, 2.0, 3.0], 0.0);
        // 6 faces * 4 vertices.
        assert_eq!(scene.vertices.len(), 24);
        assert_eq!(scene.indices.len(), 6 * 6);
        let (min, max) = scene.bounding_box().unwrap();
        assert_eq!(min, [-1.0, -2.0, -3.0]);
        assert_eq!(max, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn box_rotate_y_preserves_distances() {
        let mut scene = SceneData::default();
        add_box(&mut scene, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], 0.0);
        let dist_before: Vec<f32> = scene
            .vertices
            .iter()
            .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
            .collect();
        scene.rotate_y(0.5);
        for (i, v) in scene.vertices.iter().enumerate() {
            let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!(
                (dist - dist_before[i]).abs() < EPS,
                "distance changed at {i}"
            );
        }
        // Y coordinate must not change under Y rotation.
        assert!((scene.vertices[0][1] - 1.0).abs() < EPS);
    }

    #[test]
    fn torus_has_expected_counts() {
        let mut scene = SceneData::default();
        add_torus(&mut scene, [0.0; 3], 2.0, 0.5, 48, 24);
        assert_eq!(scene.vertices.len(), (48 + 1) * (24 + 1));
        assert_eq!(scene.indices.len(), 48 * 24 * 6);
        // Tube points stay within [major - minor, major + minor] of center.
        for v in &scene.vertices {
            let radius = (v[0] * v[0] + v[2] * v[2]).sqrt();
            assert!((radius - 2.0).abs() <= 0.5 + EPS);
        }
    }

    #[test]
    fn append_rebases_indices() {
        let mut scene = SceneData::default();
        add_box(&mut scene, [0.0; 3], [1.0; 3], 0.0);
        let first_count = scene.vertices.len() as u32;
        let mut second = SceneData::default();
        add_box(&mut second, [0.0; 3], [1.0; 3], 0.0);
        scene.append(&second);
        assert_eq!(scene.vertices.len(), 48);
        assert!(
            scene
                .indices
                .iter()
                .all(|&i| (i as usize) < scene.vertices.len())
        );
        // Second box's indices start at `first_count`, not 0.
        assert!(scene.indices.iter().any(|&i| i >= first_count));
    }

    #[test]
    fn perspective_maps_near_to_zero_far_to_one() {
        let m = perspective(60f32.to_radians(), 1.0, 0.1, 100.0);
        // Clip = M * [0, 0, z, 1] with view-space z negative (camera looks
        // down -z): ndc_z(z=-near)=0, ndc_z(z=-far)=1.
        for (z, expected) in [(-0.1f32, 0.0f32), (-100.0, 1.0)] {
            let clip_z = m[2][2] * z + m[3][2];
            let clip_w = m[2][3] * z;
            let ndc = clip_z / clip_w;
            assert!((ndc - expected).abs() < EPS, "z={z} ndc={ndc}");
        }
        // clip_w must equal -z (Vulkan convention): m[2][3] is the -1 term.
        assert!((m[2][3] + 1.0).abs() < EPS);
    }

    #[test]
    fn look_at_eye_maps_to_origin() {
        let eye = [5.0, 3.0, -4.0];
        let view = look_at(eye, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        // Applying the view matrix to the eye position yields the origin.
        let v = [
            eye[0] * view[0][0] + eye[1] * view[1][0] + eye[2] * view[2][0] + view[3][0],
            eye[0] * view[0][1] + eye[1] * view[1][1] + eye[2] * view[2][1] + view[3][1],
            eye[0] * view[0][2] + eye[1] * view[1][2] + eye[2] * view[2][2] + view[3][2],
        ];
        for c in v {
            assert!(c.abs() < EPS, "eye not mapped to origin: {v:?}");
        }
    }

    #[test]
    fn mat4_mul_matches_sequential_transform() {
        let a = perspective(60f32.to_radians(), 1.0, 0.1, 100.0);
        let b = look_at([0.0, 2.0, 4.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let combined = mat4_mul(a, b);
        let p = [0.0f32, 0.0, 0.0, 1.0];
        // Apply combined matrix to p: result[col] contributions.
        let mut via_combined = [0.0f32; 4];
        for col in 0..4 {
            for row in 0..4 {
                via_combined[row] += combined[col][row] * p[col];
            }
        }
        // Apply b then a separately.
        let mut vb = [0.0f32; 4];
        for col in 0..4 {
            for row in 0..4 {
                vb[row] += b[col][row] * p[col];
            }
        }
        let mut via_separate = [0.0f32; 4];
        for col in 0..4 {
            for row in 0..4 {
                via_separate[row] += a[col][row] * vb[col];
            }
        }
        for row in 0..4 {
            assert!(
                (via_combined[row] - via_separate[row]).abs() < EPS,
                "row {row}: {via_combined:?} vs {via_separate:?}"
            );
        }
    }
}
