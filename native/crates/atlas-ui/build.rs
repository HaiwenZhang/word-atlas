//! Register complete repository media as package resources without embedding audio bytes in Rust.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

fn collect(root: &Path, relative: &Path, out: &mut Vec<(String, String)>) -> io::Result<()> {
    let mut files = fs::read_dir(root.join(relative))?.collect::<Result<Vec<_>, _>>()?;
    files.sort_by_key(|entry| entry.file_name());
    for entry in files {
        let child = relative.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            collect(root, &child, out)?;
        } else if entry.file_type()?.is_file() {
            out.push((
                child.to_string_lossy().replace('\\', "/"),
                entry.path().to_string_lossy().into_owned(),
            ));
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let root = manifest.join("../../..").canonicalize()?;
    let public = root.join("public");
    let mut assets = Vec::new();
    for directory in ["atlas", "brand", "sfx"] {
        println!(
            "cargo:rerun-if-changed={}",
            public.join(directory).display()
        );
        collect(&public, Path::new(directory), &mut assets)?;
    }
    for (name, source) in [
        ("licenses/content.txt", root.join("CONTENT_LICENSE.md")),
        ("licenses/application.txt", root.join("LICENSE")),
        (
            "licenses/ts-fsrs.txt",
            manifest.join("../../LICENSES/ts-fsrs.txt"),
        ),
        ("licenses/nsh.txt", manifest.join("../../LICENSES/nsh.txt")),
        (
            "licenses/rusqlite.txt",
            manifest.join("../../LICENSES/rusqlite.txt"),
        ),
        (
            "licenses/sqlite.txt",
            manifest.join("../../LICENSES/sqlite.txt"),
        ),
    ] {
        println!("cargo:rerun-if-changed={}", source.display());
        assets.push((name.into(), source.to_string_lossy().into_owned()));
    }
    let output = PathBuf::from(env::var("OUT_DIR")?).join("asset-manifest.tsv");
    let contents = assets
        .iter()
        .map(|(uri, path)| format!("{uri}\t{path}\n"))
        .collect::<String>();
    fs::write(output, contents)?;
    println!("cargo:rustc-env=ATLAS_RESOURCE_COUNT={}", assets.len());
    Ok(())
}
