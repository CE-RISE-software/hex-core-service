/// Minimal caller identity passed from inbound adapters into use cases.
/// The core never validates or mints tokens — this is populated by the REST adapter.
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// JWT `sub` claim.
    pub subject: String,
    pub roles: Vec<String>,
    pub scopes: Vec<String>,
    pub tenant: Option<String>,
    /// Raw Bearer token forwarded to IO adapters. Never log this field.
    pub raw_token: Option<String>,
}
