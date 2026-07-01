//! Embedded build-time configuration for educational agent binaries.
//! Values are injected by build.rs when compiled via the dashboard build service.

/// Server URL baked in at compile time (overridden by C2_SERVER_URL env at runtime).
pub fn embedded_server_url() -> &'static str {
    option_env!("C2_EMBEDDED_SERVER_URL").unwrap_or("https://localhost:3443")
}

/// PSK baked in at compile time (overridden by C2_PSK env at runtime).
pub fn embedded_psk() -> &'static str {
    option_env!("C2_EMBEDDED_PSK").unwrap_or("educational-c2-psk-key")
}

/// Default beacon interval baked in at compile time.
pub fn embedded_beacon_interval() -> u64 {
    const RAW: Option<&str> = option_env!("C2_EMBEDDED_BEACON_INTERVAL");
    match RAW {
        Some(s) => s.parse().unwrap_or(30),
        None => 30,
    }
}
