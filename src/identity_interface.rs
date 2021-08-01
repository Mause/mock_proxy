#[derive(Debug, Clone)]
pub(super) struct Cert {
    pub(super) cert: Vec<u8>,
    pub(super) pkey: Vec<u8>,
}
impl Cert {
    pub(super) fn new(cert: Vec<u8>, pkey: Vec<u8>) -> Self {
        Self { cert, pkey }
    }
    pub(super) fn cert(&self) -> Vec<u8> {
        self.cert.clone()
    }
}

pub(super) trait IdentityInterface {
    fn mk_ca_cert(&self) -> Result<Cert, Box<dyn std::error::Error>>;
    fn mk_ca_signed_cert(
        &self,
        domain: &str,
        ca_cert: &Cert,
        password: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
}
