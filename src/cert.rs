use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, KeyPair, SanType};

pub struct Cert {
    pub cert: String,
    pub key: String,
}

pub async fn generate(alt_dnsname: &[&str]) -> Result<Cert, Box<dyn std::error::Error>> {
    let ca = Certificate::from_params(CertificateParams::from_ca_cert_pem(
        include_str!("../private/ca.pem"),
        KeyPair::from_pem(include_str!("../private/cakey.pem"))?,
    )?)?;

    let mut cert_params = CertificateParams::default();
    cert_params.distinguished_name = {
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "disable_sni_reverse_proxy");
        dn
    };
    cert_params.subject_alt_names = alt_dnsname
        .iter()
        .map(|s| SanType::DnsName(s.to_string()))
        .collect();

    let server_cert = Certificate::from_params(cert_params)?;

    Ok(Cert {
        cert: server_cert.serialize_pem_with_signer(&ca)?,
        key: server_cert.serialize_private_key_pem(),
    })
}
