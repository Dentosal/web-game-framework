use std::process::Command;

fn main() {
    let status = Command::new("wasm-pack")
        .args(["build", "--target", "web"])
        .current_dir("wasm")
        .status()
        .expect("Failed to invoke wasm-pack");

    assert!(status.success(), "Failed to compile wasm");
}
