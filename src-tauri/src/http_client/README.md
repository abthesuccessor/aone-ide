# HTTP client feature

## Responsibilities

This feature implements the API Explorer command. It validates and prepares a JSON HTTP request, applies the explicit-Send or native-confirmation authorization policy, resolves and classifies its destination, sends through a direct bounded client, redacts sensitive material, and emits observed runtime events.

## Security invariants

- Only HTTP and HTTPS are accepted; CONNECT, TRACE, fragments, URL credentials, hop-by-hop headers, oversized URLs, headers, and bodies are rejected.
- An explicit Send is sufficient for an IP-literal `DestinationScope::Loopback` target. This includes IPv4 `127.0.0.0/8` and IPv6 `::1` under the shared address classifier and requires no IDE-managed run.
- Every other target—including `localhost`, any other DNS name, and private/public non-loopback IP literals—requires native confirmation before DNS resolution or any other network activity. Confirmation fails closed after 120 seconds. Its destination is derived only from the already parsed URL and hides header/body/query values.
- After approval, DNS resolution is bounded and every returned address is classified before a connection is attempted.
- Unspecified, link-local, multicast, broadcast, cloud-metadata, documentation, transition, and other special-purpose ranges are prohibited.
- Confirmed literal private non-loopback destinations produce a pre-resolution warning. DNS names are classified after approval; private results remain allowed under that explicit destination approval. IP-literal loopback uses the explicit-Send path, while `localhost` intentionally stays on the confirmation path.
- The client disables redirects and ambient proxies. DNS names are pinned to the validated address set.
- Response bytes are bounded. Observation events retain method, sanitized URL,
  status, timing, header names, and body byte counts only; header and body values
  never enter the runtime timeline. API Explorer responses still apply the
  provider-key and sensitive-name redaction rules before IPC.

## Data structures and algorithms

- `PreparedRequest` holds the canonical method, URL, headers, JSON body, and clamped timeout.
- `ValidatedDestination` records the canonical host, deduplicated socket addresses, DNS provenance, and warning scope.
- IPv4, IPv6, mapped IPv4, and legacy compatible addresses share one explicit classification path.
- Runtime events use bounded `BTreeMap` metadata for deterministic serialization.

## Flow

1. Parse and bound the renderer request into `PreparedRequest`.
2. Treat explicit Send as authorization only for an IP-literal loopback URL; otherwise show a bounded Rust-owned native confirmation before DNS or network activity.
3. Resolve and classify every destination address.
4. Emit a value-free request event, send without proxy or redirects, then read a bounded response.
5. Emit a value-free completion or failure event and return the redacted bounded response.

## Tests

`tests.rs` covers GraphQL JSON envelopes, malformed bodies and URL credentials, header/body/query redaction, destination classification, the exact IP-literal-loopback authorization matrix, private-target warnings, confirmation timeout/decline fail-closed behavior, metadata rejection, size limits, and consent-message disclosure boundaries.
