use std::env::var;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(var("OUT_DIR").unwrap()).join("styles.css");
    println!("cargo:rerun-if-env-changed=COMPILED_CSS");
    if let Ok(path) = var("COMPILED_CSS") {
        println!("cargo:rerun-if-changed={}", path);
        std::fs::copy(path, out).unwrap();
    } else {
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
            .arg(out)
            .status()
            .unwrap()
            .success()
        {
            panic!("postcss failed");
        }
    }
}
