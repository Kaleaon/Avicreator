use avicreator_core::Vec3;
use avicreator_shape::sdf::Shape;
use avicreator_shape::{
    compile, forms, glsl, mesh, preview_in_pose, shapeshift, Influence, MeshOptions, ShapeProgram,
};

fn simple_muscle() -> ShapeProgram {
    ShapeProgram::from_json(r#"{
      "poses":{"flex":{"lower.L":{"bend":60}}},
      "parts":[
        {"kind":"chain","name":"upper","mirror":true,"from":[0.2,0,0],"points":[[1,0,0],[2,0,0]],"bones":["upper.L","lower.L"],"radius":0.05},
        {"kind":"muscle","name":"biceps","mirror":true,
         "origin":{"attach":"upper.L","at":0.1,"offset":[0,0.1,0]},
         "insertion":{"attach":"lower.L","at":0.4,"offset":[0,0.1,0]},"radius":0.12}
      ]}"#).unwrap()
}

fn belly(shape: &avicreator_shape::CompiledShape, name: &str) -> (Vec3, Vec3, f32) {
    let p = &shape.parts[shape.part_index(name).unwrap()];
    let Influence::Muscle { a, b, .. } = p.influence else {
        panic!("muscle influence")
    };
    let radius = shape.prims[p.prims.clone()]
        .iter()
        .map(|p| match p.shape {
            Shape::RoundCone { ra, rb, .. } => ra.max(rb),
            _ => panic!("tapered muscle"),
        })
        .fold(0.0, f32::max);
    (a, b, radius)
}

#[test]
fn muscles_follow_both_endpoints_mirror_and_preserve_swept_volume() {
    let mut program = simple_muscle();
    let rest = compile(&program).unwrap();
    assert_eq!(rest.bones.len(), 4, "muscles must not add deform bones");
    let (a, b, r) = belly(&rest, "biceps.L");
    assert!((r - 0.12).abs() < 1e-6);
    let (ar, br, rr) = belly(&rest, "biceps.R");
    assert!((a.x + ar.x).abs() < 1e-6 && (b.x + br.x).abs() < 1e-6);
    assert!((r - rr).abs() < 1e-6);
    program.pose = Some("flex".into());
    let posed = compile(&program).unwrap();
    let (pa, pb, pr) = belly(&posed, "biceps.L");
    assert!(pa.sub(a).length() < 1e-6, "origin stays on upper arm");
    assert!(pb.sub(b).length() > 0.1, "insertion follows forearm");
    assert!(pr > r, "shortened muscle bulges");
    assert!((pr * pr * pb.sub(pa).length() - r * r * b.sub(a).length()).abs() < 1e-5);
    let shares = avicreator_shape::field::muscle_shares(a.lerp(b, 0.5), [0, 1], a, b);
    assert!(shares.iter().all(|(_, w)| (*w - 0.5).abs() < 1e-6));
    assert_eq!(
        avicreator_shape::field::muscle_shares(a, [0, 0], a, b),
        vec![(0, 1.0)]
    );
    // Authored muscle anchors survive JSON round trips.
    assert_eq!(
        ShapeProgram::from_json(&program.to_json()).unwrap().parts,
        program.parts
    );
}

#[test]
fn muscle_validation_names_bad_endpoints_and_dimensions() {
    for (from, to, expected) in [
        ("\"at\":0.1", "\"at\":1.1", "origin: at"),
        ("\"radius\":0.12", "\"radius\":0", "radius must be positive"),
        ("\"radius\":0.12", "\"radius\":0.12,\"tendon\":0", "tendon"),
        (
            "\"attach\":\"lower.L\"",
            "\"attach\":\"missing\"",
            "insertion: unknown",
        ),
    ] {
        let p = simple_muscle().to_json().replace(' ', "").replace('\n', "");
        let p = ShapeProgram::from_json(&p.replace(from, to)).unwrap();
        let error = compile(&p).unwrap_err().to_string();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn every_form_supports_mass_definition_pose_and_skinned_export() {
    for &(name, text) in forms::FORMS {
        for (mass, definition) in [(0.5, 0.0), (1.0, 0.6), (1.6, 1.0)] {
            let p = preview_in_pose(
                text,
                &[
                    ("muscle_mass".into(), mass),
                    ("muscle_definition".into(), definition),
                ],
                Some("muscle_flex"),
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(p.shape.parts.iter().any(|p| p.kind == "muscle"));
            assert!(p.shape.prims.iter().all(|p| p
                .bounds
                .min
                .to_array()
                .into_iter()
                .chain(p.bounds.max.to_array())
                .all(f32::is_finite)));
            assert!(p.glsl.contains("smoothstep(0.0, 1.0"));
            for b in &p.shape.bones {
                let w = p.shape.weights(b.head.lerp(b.tail, 0.5));
                assert!(!w.is_empty(), "{name}: {}", b.name);
                assert!((w.iter().map(|(_, w)| w).sum::<f32>() - 1.0).abs() < 5e-4);
                assert!(w
                    .iter()
                    .all(|(b, w)| *b < p.shape.bones.len() && w.is_finite() && *w >= 0.0));
            }
        }
        let p = avicreator_shape::preview(text, &[]).unwrap();
        let mut opts = MeshOptions::new(20);
        opts.triangles = None;
        let result = mesh(&p.shape, opts);
        assert!(!result.mesh.indices.is_empty(), "{name}");
        assert!(result
            .mesh
            .positions
            .iter()
            .flat_map(|v| v.to_array())
            .all(f32::is_finite));
        assert!(result
            .mesh
            .bone_weights
            .iter()
            .all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 5e-4));
        let glb = avicreator_shape::write_glb(
            &p.shape,
            &result.mesh,
            avicreator_shape::GlbOptions {
                animation: false,
                ..Default::default()
            },
        );
        assert_eq!(&glb[..4], b"glTF");
    }
}

#[test]
fn scope_scale_applies_to_muscle_anchors_and_radius() {
    let base = simple_muscle();
    let scaled = ShapeProgram::from_json(
        &serde_json::json!({"parts":[{
            "kind":"scope", "name":"kit", "scale":2, "parts":base.parts
        }]})
        .to_string(),
    )
    .unwrap();
    let a = compile(&base).unwrap();
    let b = compile(&scaled).unwrap();
    let (a0, a1, ar) = belly(&a, "biceps.L");
    let (b0, b1, br) = belly(&b, "biceps.L");
    assert!(b0.sub(a0.scale(2.0)).length() < 1e-5 && b1.sub(a1.scale(2.0)).length() < 1e-5);
    assert!((br - 2.0 * ar).abs() < 1e-5);
}

#[test]
fn shape_shifting_preserves_endpoints_and_remeshes_different_limb_counts() {
    let human = avicreator_shape::preview(forms::get("human").unwrap(), &[])
        .unwrap()
        .shape;
    let centaur = avicreator_shape::preview(forms::get("centaur").unwrap(), &[])
        .unwrap()
        .shape;
    assert_eq!(
        shapeshift::blend_forms(&human, &centaur, 0.0).unwrap(),
        human
    );
    assert_eq!(
        shapeshift::blend_forms(&human, &centaur, 1.0).unwrap(),
        centaur
    );
    for invalid in [-0.1, 1.1, f32::NAN, f32::INFINITY] {
        assert!(shapeshift::blend_forms(&human, &centaur, invalid).is_err());
    }
    for progress in [0.25, 0.5, 0.75] {
        let s = shapeshift::blend_forms(&human, &centaur, progress).unwrap();
        let human_chest = &human.bones[human.bone_index("mChest").unwrap()];
        let horse_chest = &centaur.bones[centaur.bone_index("mChest").unwrap()];
        let chest = human_chest.head.lerp(horse_chest.head, progress);
        assert!(s.distance(chest) < 0.0, "shared torso must remain present");
        let shader = glsl::emit_glsl(&s);
        assert_eq!(shader.matches("float map(vec3 p)").count(), 1);
        assert_eq!(shader.matches("float sdRoundCone(").count(), 1);
        assert!(s.part_index("from/biceps.L").is_some());
        assert!(
            s.part_index("to/biceps.L").is_none(),
            "matching muscles must not duplicate"
        );
        assert!(s.part_index("to/horse_front.L").is_some());
        let result = mesh(&s, MeshOptions::new(20));
        assert!(!result.mesh.indices.is_empty());
        assert!(result
            .mesh
            .positions
            .iter()
            .flat_map(|v| v.to_array())
            .all(f32::is_finite));
        assert!(result
            .mesh
            .bone_weights
            .iter()
            .all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 5e-4));
        assert!(result
            .mesh
            .bone_indices
            .iter()
            .flatten()
            .all(|b| (*b as usize) < s.bones.len()));
        let mut uncull = MeshOptions::new(20);
        uncull.accelerate = false;
        let full = mesh(&s, uncull);
        assert_eq!(result.mesh.indices.len(), full.mesh.indices.len());
    }
}

#[test]
fn corresponding_parts_retarget_different_skeleton_names() {
    let make = |axis: [i32; 3], names: [&str; 2]| {
        let p = serde_json::json!({"parts":[{"kind":"chain","name":"body","from":[0,1,0],
            "dir":axis,"length":1,"segments":2,"bones":names,"radius":0.1}]})
        .to_string();
        avicreator_shape::preview(&p, &[]).unwrap().shape
    };
    let from = make([1, 0, 0], ["upper", "lower"]);
    let to = make([0, 0, 1], ["first", "second"]);
    let shifted = shapeshift::blend_forms(&from, &to, 0.5).unwrap();
    for i in 0..2 {
        assert!(
            shifted.bones[i]
                .head
                .sub(from.bones[i].head.lerp(to.bones[i].head, 0.5))
                .length()
                < 1e-6
        );
        assert!(
            shifted.bones[i]
                .tail
                .sub(from.bones[i].tail.lerp(to.bones[i].tail, 0.5))
                .length()
                < 1e-6
        );
    }
    assert_eq!(shifted.parts.len(), 1);
    let Influence::Chain { joints, .. } = &shifted.parts[0].influence else {
        panic!("chain")
    };
    assert!(joints[1].sub(shifted.bones[0].tail).length() < 1e-6);
}

#[test]
fn every_builtin_form_can_shift_from_human_with_valid_skinning() {
    let human = avicreator_shape::preview(forms::get("human").unwrap(), &[])
        .unwrap()
        .shape;
    for &(name, text) in forms::FORMS {
        let target = avicreator_shape::preview(text, &[]).unwrap().shape;
        let shifted = shapeshift::blend_forms(&human, &target, 0.5).unwrap();
        for b in &shifted.bones {
            assert!(b.parent.is_none_or(|p| p < shifted.bones.len()), "{name}");
            assert!(b.part < shifted.parts.len());
        }
        let shader = glsl::emit_glsl(&shifted);
        assert!(shader.contains("float map(vec3 p)"));
        let result = mesh(&shifted, MeshOptions::new(16));
        assert!(!result.mesh.indices.is_empty(), "{name}");
        assert!(result
            .mesh
            .positions
            .iter()
            .flat_map(|v| v.to_array())
            .all(f32::is_finite));
        assert!(result
            .mesh
            .bone_weights
            .iter()
            .all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 5e-4));
    }
}
