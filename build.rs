use std::path::PathBuf;

use clap::CommandFactory;

#[path = "src/args.rs"]
mod args;

fn generate_man_pages(out_dir: PathBuf) -> std::io::Result<()> {
    let cmd = args::Args::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    std::fs::write(out_dir.join("git-disjoint.1"), buffer)?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/args.rs");

    let out_dir =
        PathBuf::from(std::env::var_os("OUT_DIR").ok_or_else(|| std::io::ErrorKind::NotFound)?);

    generate_man_pages(out_dir)?;
    Ok(())
}
