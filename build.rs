//! Embed build metadata for diagnostic captures.

use std::fs;
use std::process::Command;

fn main() {
    rerun_if_git_changed();
    let sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GRAYCART_GIT_SHA={sha}");
}

/// `.git/HEAD` does not change on commit (it still names the same branch).
/// Watch the ref file and the HEAD log so captures track the executable that
/// was actually built.
fn rerun_if_git_changed() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/logs/HEAD");
    println!("cargo:rerun-if-changed=.git/packed-refs");
    let Ok(head) = fs::read_to_string(".git/HEAD") else {
        return;
    };
    let head = head.trim();
    if let Some(gitdir) = head.strip_prefix("gitdir: ") {
        println!("cargo:rerun-if-changed={gitdir}/HEAD");
        println!("cargo:rerun-if-changed={gitdir}/logs/HEAD");
        return;
    }
    if let Some(rel) = head.strip_prefix("ref: ") {
        println!("cargo:rerun-if-changed=.git/{rel}");
    }
}
