# Security and scope

This is an unaudited, not-formally-verified prototype. Do not treat synthetic credits or risk coefficients as a live financial system.

Trust assumptions: immutable configuration is honestly designed; registered executor/operator truthfully reports execution; the owner truthfully reports observed values; NEAR runtime/SDK behavior is correct; the deployment account's upgrade authority is controlled. The lock cannot stop external actions using bypass credentials.

Attack surfaces intentionally limited: no agent-provided deltas, no arbitrary external method or account, no dynamic policy code, no unbounded logs in state, no frontend keys, no automatic timeout releases. Limits are fixed and terminal records retained to preserve ID replay protection.

The local Rust server is a **simulator**: identities are selectable and independent-provider evidence is a button. It binds to 127.0.0.1, validates Host/Origin, requires JSON POST for API access, uses an allowlist for static files and caps requests. Do not reverse-proxy it publicly or use it as an authenticated production gateway.

The static NEAR console requires explicit wallet signatures and expected code hash. Its CDN wallet connector uses the same commit and SRI pin as trust-framework's approval UI. A matching code hash does not prove deployment immutability. Public-chain policy/position data is visible; private portfolio operation is not implemented.

Report vulnerabilities through the repository's private security reporting mechanism if enabled, or contact the repository owner privately. Do not include private keys or live account credentials in issues.
