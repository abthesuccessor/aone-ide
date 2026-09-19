# System overview projection

This feature turns persisted, evidence-backed graph facts into a bounded system overview. It selects a deterministic and diverse set of real nodes across configuration, entry, interface, application, data, and external layers, then delegates expansion to the normal bounded neighborhood traversal.

`classify.rs` owns conservative layer placement. Exact placement comes only from safe manifest filenames or existing semantic node kinds. Conventional entry, job, and generic-code placement is marked as inferred, with a human-readable basis. The projection never creates nodes and never reads or exposes `.env`, credential, cloud-provider, webhook, or schedule values.

`query.rs` owns fixed seed quotas, non-test/connected priority, deterministic ordering, limit enforcement, truncation reporting, and metadata decoration. The original node `evidence` remains unchanged; overview placement is additive metadata.
