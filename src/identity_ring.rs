use crate::Cert;
use crate::IdentityInterface;
use chrono::Local;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    RcgenError,
};
use std::ops::Add;

/// Make a CA certificate and private key
pub(super) fn mk_ca_cert() -> Result<Certificate, Box<dyn std::error::Error>> {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.name_constraints = Some(rcgen::NameConstraints {
        excluded_subtrees: vec![],
        permitted_subtrees: vec![rcgen::GeneralSubtree::DirectoryName {
            0: rcgen::DistinguishedName::new(),
        }],
    });

    Certificate::from_params(params).map_err(|f| f.into())
}

/// Make a certificate and private key signed by the given CA cert and private key
pub fn mk_ca_signed_cert(
    domain: &str,
    ca_cert: &Certificate,
    password: &str,
) -> Result<Vec<u8>, RcgenError> {
    let mut params = CertificateParams::new(vec![domain.to_owned()]);

    params.serial_number = Some(rand::random());
    params.not_before = Local::now().into();

    let date: chrono::DateTime<chrono::Utc> = Local::now().into();
    params.not_after = date.add(chrono::Duration::days(365));

    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::Any];

    let cert = Certificate::from_params(params)?;
    let cert_der = cert.serialize_der_with_signer(ca_cert)?;
    let key_der = cert.serialize_private_key_der();
    let ca_der = ca_cert.serialize_der()?;

    Ok(
        p12::PFX::new(&cert_der, &key_der, Some(&ca_der), password, "name")
            .unwrap()
            .to_der(),
    )
}

impl Cert {
    pub(crate) fn cert_string(&self) -> String {
        to_string(&self.cert)
    }
    fn pkey(&self) -> String {
        to_string(&self.pkey)
    }
}

fn to_string(thing: &[u8]) -> String {
    unsafe { String::from_utf8_unchecked(thing.to_owned()) }
}

struct RingInterface {}
impl IdentityInterface for RingInterface {
    fn mk_ca_cert(
        &self,
    ) -> std::result::Result<Cert, std::boxed::Box<(dyn std::error::Error + 'static)>> {
        let cert = mk_ca_cert()?;

        Ok(Cert::new(
            cert.serialize_pem()?.as_bytes().to_vec(),
            cert.serialize_private_key_pem().as_bytes().to_vec(),
        ))
    }
    fn mk_ca_signed_cert(
        &self,
        domain: &str,
        ca_cert: &Cert,
        password: &str,
    ) -> std::result::Result<std::vec::Vec<u8>, std::boxed::Box<(dyn std::error::Error + 'static)>>
    {
        let keypair = KeyPair::from_pem(&ca_cert.pkey())?;
        let params = CertificateParams::from_ca_cert_pem(&ca_cert.cert_string(), keypair)?;
        let certificate = Certificate::from_params(params)?;

        mk_ca_signed_cert(domain, &certificate, password).map_err(|f| f.into())
    }
}
