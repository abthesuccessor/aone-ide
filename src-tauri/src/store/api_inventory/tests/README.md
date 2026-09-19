# API inventory focused tests

This folder holds bounded fixtures that keep framework-specific source discovery out of the
already-large inventory test module. The FastAPI fixture verifies source-only discovery and exact
handler locations without requiring an OpenAPI document or starting application code.
