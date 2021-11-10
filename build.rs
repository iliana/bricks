use std::env::var;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=package-lock.json");
    println!("cargo:rerun-if-changed=postcss.config.js");
    println!("cargo:rerun-if-changed=styles.css");
    println!("cargo:rerun-if-changed=tailwind.config.js");
    println!("cargo:rerun-if-changed=templates/");

    let mut command = Command::new("npx");
    if var("PROFILE").unwrap() == "release" {
        command.env("NODE_ENV", "production");
    }
    if !command
        .arg("postcss")
        .arg("styles.css")
        .arg("-o")
        .arg(PathBuf::from(var("OUT_DIR").unwrap()).join("styles.css"))
        .status()
        .unwrap()
        .success()
    {
        panic!("postcss failed");
    }
}
