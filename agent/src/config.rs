pub fn embedded_server_url() -> &'static str {
    option_env!("C2_EMBEDDED_SERVER_URL").unwrap_or("https://localhost:3443")
}

pub fn embedded_psk() -> &'static str {
    option_env!("C2_EMBEDDED_PSK").unwrap_or("educational-c2-psk-key")
}

pub fn embedded_beacon_interval() -> u64 {
    const RAW: Option<&str> = option_env!("C2_EMBEDDED_BEACON_INTERVAL");
    match RAW {
        Some(s) => s.parse().unwrap_or(30),
        None => 30,
    }
}
