// PLAN: 1. Check license acceptance -> 2. Parse CLI args -> 3. Read source file -> 4. Transpile -> 5. Write temp runner -> 6. Execute cargo run
// Library choice: Rust standard library provides filesystem and process execution without extra dependencies.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if !license_accepted() {
        print_agpl_banner();
        if !prompt_acceptance() {
            eprintln!("Aborted.");
            std::process::exit(1);
        }
        if let Err(err) = write_acceptance_file() {
            eprintln!("Failed to record acceptance: {}", err);
            std::process::exit(1);
        }
        eprintln!("Thank you!");
    }

    let mut args: Vec<String> = env::args().skip(1).collect();
    let json_mode = args.iter().any(|arg| arg == "--json");
    args.retain(|arg| arg != "--json");

    let (command, path) = match args.get(0).map(|s| s.as_str()) {
        Some("check") | Some("eject") | Some("run") => {
            if args.len() != 2 {
                print_usage();
                std::process::exit(1);
            }
            (args[0].clone(), args[1].clone())
        }
        Some(_) => {
            if args.len() != 1 {
                print_usage();
                std::process::exit(1);
            }
            ("run".to_string(), args[0].clone())
        }
        None => {
            print_usage();
            std::process::exit(1);
        }
    };

    let path_ext = Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if path_ext != "zn" {
        eprintln!("Expected a .zn file, got: {}", path);
        std::process::exit(1);
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Failed to read {}: {}", path, err);
            std::process::exit(1);
        }
    };

    match command.as_str() {
        "check" => {
            match zinc_core::transpile_with_error(&content) {
                Ok(_) => println!("OK"),
                Err(err) => {
                    if json_mode {
                        let json = serde_json::to_string(&err)
                            .unwrap_or_else(|_| zinc_core::format_error_json("Parse failed"));
                        println!("{}", json);
                    } else {
                        eprintln!(
                            "Parse failed: {} (line {}, column {})",
                            err.message, err.line, err.column
                        );
                    }
                    std::process::exit(1);
                }
            }
        }
        "eject" => {
            let transpiled = match zinc_core::transpile_with_error(&content) {
                Ok(out) => out,
                Err(err) => {
                    eprintln!(
                        "Parse failed: {} (line {}, column {})",
                        err.message, err.line, err.column
                    );
                    std::process::exit(1);
                }
            };
            let wrapped = format!("fn main() {{\n{}\n zinc_std::check_leaks();\n}}", transpiled);
            let stem = Path::new(&path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            let out_path = Path::new(stem).with_extension("rs");
            if let Err(err) = fs::write(&out_path, wrapped) {
                eprintln!("Failed to write {}: {}", out_path.display(), err);
                std::process::exit(1);
            }
            println!("Ejected to .rs");
        }
        _ => {
            let transpiled = match zinc_core::transpile_with_error(&content) {
                Ok(out) => out,
                Err(err) => {
                    eprintln!(
                        "Parse failed: {} (line {}, column {})",
                        err.message, err.line, err.column
                    );
                    std::process::exit(1);
                }
            };
            let wrapped = format!("fn main() {{\n{}\n zinc_std::check_leaks();\n}}", transpiled);

            let temp_path = "crates/zinc_std/src/bin/temp_runner.rs";
            if let Err(err) = fs::create_dir_all("crates/zinc_std/src/bin") {
                eprintln!("Failed to create bin dir: {}", err);
                std::process::exit(1);
            }
            if let Err(err) = fs::write(temp_path, wrapped) {
                eprintln!("Failed to write {}: {}", temp_path, err);
                std::process::exit(1);
            }

            let status = Command::new("cargo")
                .args([
                    "run",
                    "--manifest-path",
                    "crates/zinc_std/Cargo.toml",
                    "--bin",
                    "temp_runner",
                ])
                .status();

            match status {
                Ok(s) if s.success() => {
                    zinc_std::check_leaks();
                }
                Ok(s) => {
                    eprintln!("temp_runner exited with status: {}", s);
                    std::process::exit(1);
                }
                Err(err) => {
                    eprintln!("Failed to run cargo: {}", err);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn acceptance_path() -> Option<PathBuf> {
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)?;
    Some(home.join(".zinc_accepted"))
}

fn license_accepted() -> bool {
    match acceptance_path() {
        Some(path) => path.exists(),
        None => false,
    }
}

fn write_acceptance_file() -> io::Result<()> {
    if let Some(path) = acceptance_path() {
        fs::write(path, "accepted")?;
    }
    Ok(())
}

fn print_agpl_banner() {
    eprintln!("----------------------------------------------------------");
    eprintln!("Zinc Language v1.0 (Fair Usage License)");
    eprintln!(" FREE: Annual Revenue < $1M USD");
    eprintln!(" PAID: $2k/yr ($1M-$5M) | $10k/yr (>$5M / Public Co)");
    eprintln!("* Revenue based on consolidated group. See COMMERCIAL_TERMS.md");
    eprintln!("----------------------------------------------------------");
    // TODO: Send heartbeat to stats.zinclang.com
}

fn prompt_acceptance() -> bool {
    eprint!("Do you agree to the AGPL v3 terms? [y/N]: ");
    let _ = io::stderr().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    matches!(input.trim(), "y" | "Y")
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  zn run <path>.zn");
    eprintln!("  zn check <path>.zn [--json]");
    eprintln!("  zn eject <path>.zn");
}
