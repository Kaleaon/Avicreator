//! `avishape` — check, build and live-edit rig-aware shape programs.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use avicreator_shape::{gltf, glsl, mesh, mesh_options, preview, sl, studio, GlbOptions, SkeletonTarget};

const USAGE: &str = "\
avishape — rig-aware implicit shape programs

USAGE:
  avishape check  <program.json> [--set NAME=VALUE]...
  avishape build  <program.json> [--out DIR] [--resolution N] [--triangles N] [--set NAME=VALUE]... [--no-anim]
  avishape studio <program.json> [--port PORT]

check   Validate the program and list its parts, bones and sizes (no meshing).
build   Write <name>.glb (skinned mesh + skeleton + 'flex' clip), <name>.html
        (standalone raymarched preview) and <name>.glsl into DIR (default: .).
        Programs targeting the second_life skeleton also get <name>.dae, rigged
        to SL joints for the Second Life mesh uploader.
studio  Serve Shape Studio on http://127.0.0.1:PORT (default 8765): live preview,
        parameter sliders, influence painting, .glb export, and reload when the
        file changes on disk.";

struct Args {
    command: String,
    program: PathBuf,
    out: PathBuf,
    resolution: Option<u32>,
    triangles: Option<usize>,
    sets: Vec<(String, f32)>,
    animation: bool,
    port: u16,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let command = it.next().ok_or("missing command")?;
    if command == "-h" || command == "--help" || command == "help" {
        return Err(String::new());
    }
    let program = PathBuf::from(it.next().ok_or("missing program path")?);
    let mut args = Args { command, program, out: PathBuf::from("."), resolution: None, triangles: None, sets: Vec::new(), animation: true, port: 8765 };
    while let Some(flag) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{} needs a value", name));
        match flag.as_str() {
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
    let p = preview(&text, &args.sets).map_err(|e| e.to_string())?;
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
            let html_path = write("html", glsl::viewer_html(Some((&p.glsl, &p.meta))).as_bytes())?;
            let mut opts = mesh_options(&p.shape, args.resolution);
            if args.triangles.is_some() {
                opts.triangles = args.triangles;
            }
            let result = mesh(&p.shape, opts);
            let stats = &result.stats;
            let glb = gltf::write_glb(&p.shape, &result.mesh, GlbOptions { animation: args.animation, ..GlbOptions::default() });
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
