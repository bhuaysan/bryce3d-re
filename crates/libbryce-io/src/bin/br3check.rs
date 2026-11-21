// SPDX-License-Identifier: GPL-3.0-or-later
//! Validate Bryce composite files, one line per file (counterpart of `br3dump.py --check`).
//! Usage: br3check [--tree] FILE...

use bryce_io::scene::{Br3File, Node};

fn show(f: &Br3File, nodes: &[Node], ind: usize) {
    for n in nodes {
        let mut extra = String::new();
        if let Some((t, r)) = f.find(n.identity, n.type_name.as_deref()) {
            if let Some(o) = f.object(t, r) {
                let p = o.position();
                extra = format!("  pos=({:.3}, {:.3}, {:.3})", p[0], p[1], p[2]);
                if let Some((_, m)) = o.material.and_then(|m| f.find(m, Some("C3dBaseMaterial"))) {
                    extra += &format!(" material='{}'", m.name);
                }
                if !o.channels.is_empty() {
                    let names: Vec<_> = o.channels.iter().map(|c| c.0.split("_e").nth(1).unwrap_or(c.0)).collect();
                    extra += &format!(" anim={}", names.join(","));
                }
            }
        }
        println!("{}  {}/{}  '{}' ({}){extra}", "    ".repeat(ind), n.class, n.subclass,
                 n.name.as_deref().unwrap_or(""), n.type_name.as_deref().unwrap_or("?"));
        show(f, &n.children, ind + 1);
    }
}

fn main() {
    let mut tree = false;
    let mut rc = 0;
    for arg in std::env::args().skip(1) {
        if arg == "--tree" {
            tree = true;
            continue;
        }
        match Br3File::open(&arg) {
            Ok(mut f) => {
                let nodes = f.scene_tree();
                let warn = !f.warnings.is_empty();
                println!("{} {arg}: v{} {} entries, {} types, {} resources{}",
                         if warn { "WARN" } else { "OK  " },
                         f.cmf.version.map_or("?".into(), |v| v.to_string()),
                         f.cmf.used_count, f.types.len(), f.resource_count(),
                         if warn { format!(" ({})", f.warnings.join("; ")) } else { String::new() });
                if tree {
                    show(&f, &nodes, 1);
                }
                if warn {
                    rc = rc.max(2);
                }
            }
            Err(e) => {
                println!("FAIL {arg}: {e}");
                rc = 1;
            }
        }
    }
    std::process::exit(rc);
}
