use std::process::Command;

fn main() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let build_date = chrono::Utc::now().format("%Y-%m-%d").to_string();

    println!("cargo:rustc-env=RETROTOOLS_GIT_HASH={}", git_hash);
    println!("cargo:rustc-env=RETROTOOLS_BUILD_DATE={}", build_date);
    println!("cargo:rerun-if-changed=build.rs");
}
