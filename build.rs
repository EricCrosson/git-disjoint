use std::path::{Path, PathBuf};

use clap::{CommandFactory, ValueEnum};
use clap_complete::{generate_to, Shell};

#[path = "src/cli.rs"]
mod cli;

fn generate_man_pages(out_dir: &Path, command: clap::Command) -> std::io::Result<()> {
    let man = clap_mangen::Man::new(command);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    std::fs::write(out_dir.join("git-disjoint.1"), buffer)?;
    Ok(())
}

fn generate_shell_completions(out_dir: &Path, mut command: clap::Command) -> std::io::Result<()> {
    Shell::value_variants().iter().try_for_each(|shell| {
        generate_to(*shell, &mut command, "git-disjoint", out_dir)?;
        Ok(())
    })
}

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/args.rs");

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").ok_or(std::io::ErrorKind::NotFound)?);
    let command = cli::Cli::command();

    let profile = std::env::var("PROFILE").expect("cargo should set PROFILE environment variable");
    if profile == "release" {
        generate_man_pages(&out_dir, command.clone())?;
        generate_shell_completions(&out_dir, command)?;
    }

    Ok(())
}
