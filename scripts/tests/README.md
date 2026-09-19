# Script tests

Node's built-in test runner exercises release-path confinement, proof binding,
checksum integrity, manifest promotion, and fail-closed public-release behavior.
The suite is intentionally independent of Vitest and runs through
`npm run test:release-security`.
