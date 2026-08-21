# Security Policy

The Fasti project takes the security and privacy of your media chronicle seriously. This document outlines our vulnerability disclosure process, threat model, and built-in security guarantees.

---

## 1. Supported Versions

We provide security updates and patches for the following versions:

| Version | Supported |
|---|---|
| `0.1.x` (Current development) | :white_check_mark: |
| `< 0.1.0` | :x: |

---

## 2. Reporting a Vulnerability

If you discover or suspect a security vulnerability in Fasti:

1. **Do NOT open a public GitHub issue.**
2. Report the vulnerability privately via GitHub Security Advisories at [https://github.com/Scrobble-dev/Fasti/security/advisories/new](https://github.com/Scrobble-dev/Fasti/security/advisories/new) or by emailing the security team at **`security@scrobble.dev`**.
3. Please include in your report:
   * A description of the vulnerability and its potential impact.
   * Step-by-step reproduction instructions or a minimal proof of concept.
   * The affected component (`fastid`, `fasti-store`, `fasti-sync`, web UI, Tauri desktop, etc.).
   * Any proposed mitigations or patches if available.

### Response Timeline
* **Initial Response:** Within 48 hours acknowledging receipt of the report.
* **Triage & Assessment:** Within 7 days with a severity evaluation and remediation timeline.
* **Coordinated Disclosure:** We will work with the reporter on a mutually agreed timeline before publicly disclosing the vulnerability and publishing patched builds.

---

## 3. Core Security & Privacy Invariants

Fasti is designed with a defense-in-depth security posture tailored for self-hosted environments:

```
[Untrusted Network]
         │
         ▼ (Firewall / Reverse Proxy)
┌────────────────────────────────────────────────────────┐
│ Fasti Security Perimeter                               │
│                                                        │
│  [1. Strict Loopback / Private Bind by Default]        │
│  [2. Scoped API Tokens with Capability Boundaries]     │
│  [3. WebAuthn / Passkeys for Operator Authentication]  │
│  [4. Zero Outbound Telemetry / Zero Third-Party Phone] │
│  [5. Sanitized Connector Metadata Ingestion]           │
│                                                        │
└────────────────────────────────────────────────────────┘
```

1. **Private by Default:**
   * `fastid` binds to `127.0.0.1` by default unless explicitly configured for network exposure behind a verified reverse proxy or VPN.
   * Registration is closed automatically after the initial owner account bootstrap.
2. **Zero Telemetry:**
   * Fasti contains no phone-home analytics, user tracking, or hidden telemetry reporting. Crash diagnostics are purely local and opt-in.
3. **Least-Privilege Scoping:**
   * API tokens are strictly scoped (e.g., `events:write`, `history:read`, `admin:sync`).
   * Desktop webviews running under Tauri v2 operate with locked-down capabilities without unrestricted shell or filesystem access.
4. **SSRF & Connector Hardening:**
   * External metadata connectors validate URLs against strict allowlists, rejecting private subnet ranges (`10.0.0.0/8`, `192.168.0.0/16`, `127.0.0.0/8`, link-local) to prevent Server-Side Request Forgery.
5. **Secure Cryptographic Storage:**
   * Passwords and credentials use memory-hard hashing (Argon2id) compliant with RFC 9106.
   * Database backups and exports cleanly separate personal credentials from portable media history.
