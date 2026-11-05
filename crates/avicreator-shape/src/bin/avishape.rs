//! `avishape` — check, build and live-edit rig-aware shape programs.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use avicreator_shape::reference::{self, Landmarks, Placement, Reference, TriangleGrid};
use avicreator_shape::{gltf, glsl, mesh, mesh_options, preview_in_pose, sl, studio, GlbOptions, SkeletonTarget};

const USAGE: &str = "\
avishape — rig-aware implicit shape programs

USAGE:
  avishape check  <program.json> [--set NAME=VALUE]...
  avishape build  <program.json> [--out DIR] [--resolution N] [--triangles N] [--set NAME=VALUE]... [--no-anim]
  avishape studio <program.json> [--port PORT]
  avishape compare <program.json> <reference.obj|glb> [PLACEMENT] [--samples N] [--set NAME=VALUE]...
  avishape fit     <program.json> <reference.obj|glb> [PLACEMENT] [--params a,b,..] [--evals N] [--write]

PLACEMENT: [--scale S] [--rotate-y DEGREES] [--ground] [--match-height] [--landmarks FILE.json]
Any command takes --pose NAME to compile in one of the program's named poses (e.g. to
match a reference standing in anatomical position rather than T-pose).
check/build also take --target FORM_OR_JSON --progress 0..1 to shape-shift.
Built-in forms: human, canine, feline, vulpine, minotaur, centaur, scaly, lizard, dragon.

check   Validate the program and list its parts, bones and sizes (no meshing).
build   Write <name>.glb (skinned mesh + skeleton + 'flex' clip), <name>.html
        (standalone raymarched preview) and <name>.glsl into DIR (default: .).
        Programs targeting the second_life skeleton also get <name>.dae, rigged
        to SL joints for the Second Life mesh uploader.
studio  Serve Shape Studio on http://127.0.0.1:PORT (default 8765): live preview,
        parameter sliders, influence painting, .glb export, and reload when the
        file changes on disk.
compare Measure a program against a reference mesh (MakeHuman OBJ, glTF character):
        surface distances both ways, and per part whether it is too big or small.
fit     Move the program's params (all ranged params by default) within their
        ranges to match the reference; --write saves the values into the file.
        References are only measured; no geometry is copied into the program.";

struct Args {
    command: String,
    program: PathBuf,
    out: PathBuf,
    resolution: Option<u32>,
    triangles: Option<usize>,
    sets: Vec<(String, f32)>,
    animation: bool,
    port: u16,
    reference: Option<PathBuf>,
    placement: Placement,
    samples: usize,
    params: Option<Vec<String>>,
    evals: usize,
    write: bool,
    match_height: bool,
    landmarks: Option<PathBuf>,
    pose: Option<String>,
    target: Option<String>,
    progress: f32,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let command = it.next().ok_or("missing command")?;
    if command == "-h" || command == "--help" || command == "help" {
        return Err(String::new());
    }
    let program = PathBuf::from(it.next().ok_or("missing program path")?);
    let reference = if command == "compare" || command == "fit" {
        Some(PathBuf::from(it.next().ok_or("missing reference mesh path")?))
    } else {
        None
    };
    let mut args = Args {
        command,
        program,
        out: PathBuf::from("."),
        resolution: None,
        triangles: None,
        sets: Vec::new(),
        animation: true,
        port: 8765,
        reference,
        placement: Placement::default(),
        samples: 4000,
        params: None,
        evals: 200,
        write: false,
        match_height: false,
        landmarks: None,
        pose: None,
        target: None,
        progress: 0.0,
    };
    while let Some(flag) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{} needs a value", name));
        match flag.as_str() {
            "--target" => args.target = Some(value("--target")?),
            "--progress" => {
                args.progress = value("--progress")?.parse().map_err(|_| "--progress must be a number within 0..1")?;
                if !args.progress.is_finite() || !(0.0..=1.0).contains(&args.progress) { return Err("--progress must be finite and within 0..1".into()); }
            }
            "--out" => args.out = PathBuf::from(value("--out")?),
            "--resolution" => {
                let r: u32 = value("--resolution")?.parse().map_err(|_| "--resolution must be a whole number")?;
                if !(8..=avicreator_shape::mesher::MAX_RESOLUTION).contains(&r) {
                    return Err(format!("--resolution must be between 8 and {}", avicreator_shape::mesher::MAX_RESOLUTION));
                }
                args.resolution = Some(r);
            }
            "--triangles" => {
                let t: usize = value("--triangles")?.parse().map_err(|_| "--triangles must be a whole number")?;
                args.triangles = Some(t.max(100));
            }
            "--set" => {
                let kv = value("--set")?;
                let (k, v) = kv.split_once('=').ok_or("--set expects NAME=VALUE")?;
                let v: f32 = v.parse().map_err(|_| format!("--set {}: '{}' is not a number", k, v))?;
                args.sets.push((k.to_string(), v));
            }
            "--no-anim" => args.animation = false,
            "--scale" => args.placement.scale = value("--scale")?.parse().map_err(|_| "--scale must be a number")?,
            "--rotate-y" => {
                args.placement.rotate_y_degrees = value("--rotate-y")?.parse().map_err(|_| "--rotate-y must be a number")?
            }
            "--ground" => args.placement.ground = true,
            "--match-height" => args.match_height = true,
            "--landmarks" => args.landmarks = Some(PathBuf::from(value("--landmarks")?)),
            "--pose" => args.pose = Some(value("--pose")?),
            "--samples" => args.samples = value("--samples")?.parse().map_err(|_| "--samples must be a whole number")?,
            "--params" => args.params = Some(value("--params")?.split(',').map(|p| p.trim().to_string()).collect()),
            "--evals" => args.evals = value("--evals")?.parse().map_err(|_| "--evals must be a whole number")?,
            "--write" => args.write = true,
            "--port" => args.port = value("--port")?.parse().map_err(|_| "--port must be a number")?,
            other => return Err(format!("unknown option '{}'", other)),
        }
    }
    Ok(args)
}

fn kib(bytes: usize) -> String {
    format!("{:.1} KiB", bytes as f64 / 1024.0)
}

fn run(args: Args) -> Result<(), String> {
    let text = std::fs::read_to_string(&args.program).map_err(|e| format!("{}: {}", args.program.display(), e))?;
    let started = Instant::now();
    let p = if let Some(target) = &args.target {
        if !matches!(args.command.as_str(), "check" | "build") { return Err("--target is supported by check and build".into()); }
        let target = match avicreator_shape::forms::get(target) {
            Some(text) => text.to_string(),
            None => std::fs::read_to_string(target).map_err(|e| format!("target {}: {}", target, e))?,
        };
        let mut program = avicreator_shape::ShapeProgram::from_json(&text).map_err(|e| e.to_string())?;
        for (name, value) in &args.sets { program.set_param(name, *value).map_err(|e| e.to_string())?; }
        avicreator_shape::shapeshift::preview_shift(&program.to_json(), &target, args.progress, args.pose.as_deref()).map_err(|e| e.to_string())?
    } else { preview_in_pose(&text, &args.sets, args.pose.as_deref()).map_err(|e| e.to_string())? };
    let compile_ms = started.elapsed().as_secs_f64() * 1000.0;

    match args.command.as_str() {
        "check" => {
            let target = match p.shape.skeleton {
                SkeletonTarget::SecondLife => " (Second Life skeleton: names and parenting valid)",
                SkeletonTarget::Custom => "",
            };
            println!("{}: ok ({:.1} ms){}", p.shape.name, compile_ms, target);
            println!("  program {} -> GLSL {}", kib(text.len()), kib(p.glsl.len()));
            println!("  {} parts, {} primitives, {} bones", p.shape.parts.len(), p.shape.prims.len(), p.shape.bones.len());
            for part in &p.shape.parts {
                let bones: Vec<&str> =
                    p.shape.bones.iter().filter(|b| p.shape.parts[b.part].name == part.name).map(|b| b.name.as_str()).collect();
                println!("  - {:<18} {:<9} {:>3} prims  {}", part.name, part.kind, part.prims.len(), bones.join(" "));
            }
            Ok(())
        }
        "build" => {
            std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
            let stem = args.program.file_stem().and_then(|s| s.to_str()).unwrap_or("shape");
            let write = |ext: &str, bytes: &[u8]| -> Result<PathBuf, String> {
                let path = args.out.join(format!("{}.{}", stem, ext));
                std::fs::write(&path, bytes).map_err(|e| format!("{}: {}", path.display(), e))?;
                Ok(path)
            };
            let glsl_path = write("glsl", p.glsl.as_bytes())?;
            let mut opts = mesh_options(&p.shape, args.resolution);
            if args.triangles.is_some() {
                opts.triangles = args.triangles;
            }
            let result = mesh(&p.shape, opts);
            let html_path = write("html", glsl::viewer_mesh_html(&p.meta, &glsl::mesh_payload(&p.shape, &result.mesh)).as_bytes())?;
            let stats = &result.stats;
            let glb = gltf::write_glb(&p.shape, &result.mesh, GlbOptions { animation: args.animation && p.shape.transition.is_none(), ..GlbOptions::default() });
            let glb_path = write("glb", &glb)?;
            let dae = (p.shape.skeleton == SkeletonTarget::SecondLife)
                .then(|| write("dae", sl::write_collada(&p.shape, &result.mesh).as_bytes()))
                .transpose()?;
            println!("{}: {} parts, {} bones", p.shape.name, p.shape.parts.len(), p.shape.bones.len());
            println!("  program   {:>10}", kib(text.len()));
            println!("  glsl      {:>10}  {}", kib(p.glsl.len()), glsl_path.display());
            println!("  preview   {:>10}  {}", "", html_path.display());
            println!("  mesh      {:>10}  {}", kib(glb.len()), glb_path.display());
            println!(
                "            {} verts, {} tris{}; grid {}x{}x{} ({:.1} mm cells), {:.1}% of samples evaluated, {:.0} ms",
                stats.vertices,
                stats.triangles,
                if stats.raw_triangles != stats.triangles {
                    format!(" (simplified from {} in {:.0} ms)", stats.raw_triangles, stats.simplify_millis)
                } else {
                    String::new()
                },
                stats.grid[0],
                stats.grid[1],
                stats.grid[2],
                stats.cell_size * 1000.0,
                100.0 * stats.samples_evaluated as f64 / stats.samples_total.max(1) as f64,
                stats.millis
            );
            if let Some(dae) = dae {
                println!("  second life           {}", dae.display());
                println!("            upload as a rigged mesh with \"Include joint positions\" enabled");
                if stats.vertices > 65_535 {
                    println!("            note: over 65,535 vertices; lower --triangles for a single-face upload");
                }
            }
            Ok(())
        }
        "compare" | "fit" => {
            let path = args.reference.as_ref().unwrap();
            let mut placement = args.placement;
            if args.match_height {
                placement.height = Some(p.shape.bounds.size().y - 2.0 * p.shape.settings.blend);
            }
            let reference = Reference::load(path, placement)?;
            if !reference.dropped.is_empty() {
                println!("ignored outlier meshes far outside the figure: {}", reference.dropped.join(", "));
            }
            let rb = reference.bounds();
            let sb = p.shape.bounds;
            let (rs, ss) = (rb.size(), sb.size());
            println!(
                "reference {}: {} triangles, size {:.3} x {:.3} x {:.3} m (x, y up, z forward), y {:.3}..{:.3}",
                display(path),
                reference.triangles.len(),
                rs.x,
                rs.y,
                rs.z,
                rb.min.y,
                rb.max.y
            );
            println!("program: size {:.3} x {:.3} x {:.3} m, y {:.3}..{:.3}", ss.x, ss.y, ss.z, sb.min.y, sb.max.y);
            let landmarks = match &args.landmarks {
                Some(file) => Some(Landmarks::load(file, &reference)?),
                None => None,
            };
            let grid = TriangleGrid::new(&reference.triangles);
            let samples = reference::sample_surface(&reference.triangles, args.samples, 0xa11ce);
            let resolution = args.resolution.unwrap_or(96);
            let mut program = p.program.clone();
            if args.command == "fit" {
                let names: Vec<String> = match &args.params {
                    Some(list) => list.clone(),
                    None => program
                        .params
                        .iter()
                        .filter(|(_, v)| matches!(v, avicreator_shape::Param::Ranged { .. }))
                        .map(|(k, _)| k.clone())
                        .collect(),
                };
                if names.is_empty() {
                    return Err("no params to fit (declare ranges or pass --params)".into());
                }
                println!("fitting {} params: {}", names.len(), names.join(", "));
                let result = reference::fit(&mut program, &names, &grid, &samples, landmarks.as_ref(), resolution, args.evals, |evals, score| {
                    println!("  eval {:>4}: objective {:.2} mm", evals, score * 1000.0);
                })?;
                println!(
                    "objective (two-way mean surface distance{}) {:.2} mm -> {:.2} mm in {} evaluations",
                    if landmarks.is_some() { " + mean landmark error" } else { "" },
                    result.before * 1000.0,
                    result.after * 1000.0,
                    result.evaluations
                );
                for (name, before, after) in &result.params {
                    println!("  {:<18} {:>9.4} -> {:>9.4}", name, before, after);
                }
                if args.write {
                    let updated = avicreator_shape::program::set_param_values(
                        &text,
                        &result.params.iter().map(|(n, _, v)| (n.clone(), *v)).collect::<Vec<_>>(),
                    )
                    .map_err(|e| e.to_string())?;
                    std::fs::write(&args.program, updated).map_err(|e| e.to_string())?;
                    println!("wrote fitted values into {}", args.program.display());
                } else {
                    let sets: Vec<String> = result.params.iter().map(|(n, _, v)| format!("--set {}={:.4}", n, v)).collect();
                    println!("apply with: {}  (or rerun with --write)", sets.join(" "));
                }
            }
            let shape = avicreator_shape::compile(&program).map_err(|e| e.to_string())?;
            let report = reference::compare(&shape, &grid, &samples, resolution, true);
            let mm = |v: f32| v * 1000.0;
            println!("reference -> program: mean {:.1} mm, rms {:.1} mm, p95 {:.1} mm, max {:.1} mm",
                mm(report.reference_to_shape.mean_abs), mm(report.reference_to_shape.rms), mm(report.reference_to_shape.p95), mm(report.reference_to_shape.max));
            println!("program -> reference: mean {:.1} mm, rms {:.1} mm, p95 {:.1} mm, max {:.1} mm",
                mm(report.shape_to_reference.mean_abs), mm(report.shape_to_reference.rms), mm(report.shape_to_reference.p95), mm(report.shape_to_reference.max));
            println!("  {:<22} {:>12} {:>12}   (offset > 0: reference sticks out, part too small)", "part", "offset mm", "gap mm");
            let mut parts: Vec<_> = report.parts.iter().filter(|r| r.reference_samples + r.surface_samples > 0).collect();
            parts.sort_by(|a, b| (b.surface_gap + b.reference_offset.abs()).total_cmp(&(a.surface_gap + a.reference_offset.abs())));
            for r in parts {
                println!("  {:<22} {:>+12.1} {:>12.1}", r.name, mm(r.reference_offset), mm(r.surface_gap));
            }
            if let Some(l) = &landmarks {
                println!("landmarks (mean error {:.1} mm):", mm(l.error(&shape)));
                for (name, target) in &l.bones {
                    if let Some(b) = shape.bone_index(name) {
                        let d = shape.bones[b].head.sub(*target);
                        println!("  {:<22} {:>7.1} mm off   (dx {:+.1}, dy {:+.1}, dz {:+.1})", name, mm(d.length()), mm(d.x), mm(d.y), mm(d.z));
                    }
                }
                for (name, target) in &l.surface {
                    println!("  {:<22} {:>+7.1} mm   (surface distance; > 0 means the program surface is short of it)", name, mm(shape.distance(*target)));
                }
            }
            Ok(())
        }
        "studio" => {
            let addr = format!("127.0.0.1:{}", args.port);
            let listener = TcpListener::bind(&addr).map_err(|e| format!("cannot listen on {}: {}", addr, e))?;
            println!("Shape Studio: http://{}  (editing {})", addr, display(&args.program));
            println!("  {} parts, {} bones. Ctrl+C to stop.", p.shape.parts.len(), p.shape.bones.len());
            studio::serve(args.program.clone(), listener).map_err(|e| e.to_string())
        }
        other => Err(format!("unknown command '{}'", other)),
    }
}

fn display(path: &Path) -> String {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf()).display().to_string()
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) if e.is_empty() => {
            println!("{}", USAGE);
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("error: {}\n\n{}", e, USAGE);
            return ExitCode::FAILURE;
        }
    };
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}
