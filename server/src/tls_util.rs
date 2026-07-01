use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use std::fs;
use std::path::Path;

pub fn ensure_certs(cert_dir: &str) -> Result<(String, String), String> {
    let cert_path = format!("{}/cert.pem", cert_dir);
    let key_path = format!("{}/key.pem", cert_dir);

    if Path::new(&cert_path).exists() && Path::new(&key_path).exists() {
        return Ok((cert_path, key_path));
    }

    fs::create_dir_all(cert_dir).map_err(|e| e.to_string())?;

    let key_pair = KeyPair::generate().map_err(|e| e.to_string())?;
    let mut params = CertificateParams::new(vec!["localhost".to_string()])
        .map_err(|e| e.to_string())?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "Educational C2 Server");
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(365);

    let cert = params.self_signed(&key_pair).map_err(|e| e.to_string())?;
    fs::write(&cert_path, cert.pem()).map_err(|e| e.to_string())?;
    fs::write(&key_path, key_pair.serialize_pem()).map_err(|e| e.to_string())?;

    Ok((cert_path, key_path))
}
