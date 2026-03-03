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

#[cfg(test)]
mod tests {
    use super::SecurityContext;

    #[test]
    fn security_context_clone_preserves_fields() {
        let ctx = SecurityContext {
            subject: "user-123".into(),
            roles: vec!["admin".into()],
            scopes: vec!["records:write".into()],
            tenant: Some("tenant-a".into()),
            raw_token: Some("jwt-token".into()),
        };

        let cloned = ctx.clone();
        assert_eq!(cloned.subject, "user-123");
        assert_eq!(cloned.roles, vec!["admin"]);
        assert_eq!(cloned.scopes, vec!["records:write"]);
        assert_eq!(cloned.tenant.as_deref(), Some("tenant-a"));
        assert_eq!(cloned.raw_token.as_deref(), Some("jwt-token"));
    }
}
