fn main() {
    let version = rustc_version::version()
        .map(|e| e.to_string())
        .unwrap_or_else(|_| "0.0".to_string());

    println!("cargo:rustc-env=RUSTC_VERSION={}", version);
}
