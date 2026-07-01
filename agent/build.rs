//! Compile-time configuration injection for educational agent builds.
//! The build service sets C2_BUILD_* env vars before invoking `cargo build`.

fn main() {
    let server_url = std::env::var("C2_BUILD_SERVER_URL")
        .unwrap_or_else(|_| "https://localhost:3443".to_string());
    let psk = std::env::var("C2_BUILD_PSK")
        .unwrap_or_else(|_| "educational-c2-psk-key".to_string());
    let beacon_interval = std::env::var("C2_BUILD_BEACON_INTERVAL")
        .unwrap_or_else(|_| "30".to_string());

    println!("cargo:rustc-env=C2_EMBEDDED_SERVER_URL={server_url}");
    println!("cargo:rustc-env=C2_EMBEDDED_PSK={psk}");
    println!("cargo:rustc-env=C2_EMBEDDED_BEACON_INTERVAL={beacon_interval}");
    println!("cargo:rerun-if-env-changed=C2_BUILD_SERVER_URL");
    println!("cargo:rerun-if-env-changed=C2_BUILD_PSK");
    println!("cargo:rerun-if-env-changed=C2_BUILD_BEACON_INTERVAL");
}
