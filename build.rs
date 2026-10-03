use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=flake.nix");
    println!("cargo:rerun-if-changed=flake.lock");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") || !is_nix_family() {
        return;
    }

    let system = match env::var("TARGET") {
        Ok(target) if target.starts_with("aarch64") => "aarch64-linux",
        _ => "x86_64-linux",
    };
    let attribute = format!(".#libPath.{system}");
    let output = Command::new("nix")
        .args(["eval", "--raw", &attribute])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout);
            let path = path.trim();
            if !path.is_empty() {
                // Floem/winit loads Wayland/X11 libraries dynamically. Embedding
                // the locked flake's library path makes direct Cargo binaries
                // work on NixOS/UmbraOS without an interactive nix develop shell.
                println!("cargo:rustc-link-arg=-Wl,-rpath,{path}");
            }
        }
        Ok(output) => println!(
            "cargo:warning=Could not resolve Umbra Note runtime libraries: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => {
            println!("cargo:warning=Nix was not available for runtime library discovery: {error}")
        }
    }
}

fn is_nix_family() -> bool {
    std::fs::read_to_string("/etc/os-release").is_ok_and(|release| {
        release.lines().any(|line| {
            line == "ID=nixos"
                || line == "ID=umbra"
                || line == "ID_LIKE=nixos"
                || line
                    .strip_prefix("ID_LIKE=")
                    .is_some_and(|ids| ids.split_whitespace().any(|id| id == "nixos"))
        })
    })
}
