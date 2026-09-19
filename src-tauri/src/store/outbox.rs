//! Transactional outbox.
//!
//! Analysis results and the events that announce them must agree. Emitting an
//! event directly from a mutation lets the two diverge: the process can exit
//! between a committed transaction and a delivered event, or an event can be
//! published for a transaction that later rolls back. Recording the event as a
//! row inside the same transaction removes both cases — the event exists if and
//! only if the data change it describes was committed.
//!
//! Delivery is therefore at-least-once and strictly ordered. A consumer marks a
//! contiguous prefix delivered only after it has handled it, so an interrupted
//! drain repeats work rather than losing it. Every consumer must be idempotent.

use rusqlite::{Connection, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::error::{AoneError, AoneResult};

/// Payloads are bounded like every other persisted fact in the store. An event
/// is a notification, not a transport for file content.
pub(super) const MAX_PAYLOAD_BYTES: usize = 16 * 1024;
/// Upper bound on rows returned by one drain, so a long backlog cannot produce
/// an unbounded allocation or an oversized IPC message.
pub(super) const MAX_DRAIN_RECORDS: usize = 500;
/// Delivered rows are pruned beyond this many, keeping the table from growing
/// without bound across a long session while retaining recent history.
pub(super) const MAX_RETAINED_DELIVERED: usize = 1_000;

pub(super) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS outbox (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    recorded_at TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    delivered_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_outbox_pending
    ON outbox(sequence) WHERE delivered_at IS NULL;
"#;

/// One durable, ordered record of something that happened to the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxRecord {
    pub sequence: i64,
    pub recorded_at: String,
    pub kind: String,
    pub payload: String,
}

/// Appends an event to the outbox **within the caller's transaction**. Taking a
/// `&Transaction` rather than a `&Connection` is the whole contract: it is not
/// possible to record an event outside the transaction that produced it.
pub(super) fn enqueue(transaction: &Transaction<'_>, kind: &str, payload: &str) -> AoneResult<i64> {
    validate(kind, payload)?;
    transaction.execute(
        "INSERT INTO outbox (recorded_at, kind, payload, delivered_at)
         VALUES (?1, ?2, ?3, NULL)",
        params![now(), kind, payload],
    )?;
    Ok(transaction.last_insert_rowid())
}

/// Returns the oldest undelivered records, in commit order, bounded by `limit`.
/// Records stay pending until [`mark_delivered`] is called, so a consumer that
/// fails midway sees them again.
pub(super) fn pending(connection: &Connection, limit: usize) -> AoneResult<Vec<OutboxRecord>> {
    let limit = limit.clamp(1, MAX_DRAIN_RECORDS);
    let mut statement = connection.prepare(
        "SELECT sequence, recorded_at, kind, payload
         FROM outbox
         WHERE delivered_at IS NULL
         ORDER BY sequence
         LIMIT ?1",
    )?;
    let records = statement
        .query_map(params![limit as i64], |row| {
            Ok(OutboxRecord {
                sequence: row.get(0)?,
                recorded_at: row.get(1)?,
                kind: row.get(2)?,
                payload: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(records)
}

/// Marks every record up to and including `sequence` delivered. Callers pass
/// the last sequence they actually handled, so a partial drain advances only as
/// far as the work that succeeded.
pub(super) fn mark_delivered(connection: &Connection, sequence: i64) -> AoneResult<usize> {
    let delivered = connection.execute(
        "UPDATE outbox SET delivered_at = ?1
         WHERE sequence <= ?2 AND delivered_at IS NULL",
        params![now(), sequence],
    )?;
    prune_delivered(connection)?;
    Ok(delivered)
}

/// Drops the oldest delivered rows once history exceeds the retention bound.
/// Pending rows are never removed, however far behind a consumer has fallen.
fn prune_delivered(connection: &Connection) -> AoneResult<usize> {
    let removed = connection.execute(
        "DELETE FROM outbox
         WHERE delivered_at IS NOT NULL
           AND sequence NOT IN (
               SELECT sequence FROM outbox
               WHERE delivered_at IS NOT NULL
               ORDER BY sequence DESC
               LIMIT ?1
           )",
        params![MAX_RETAINED_DELIVERED as i64],
    )?;
    Ok(removed)
}

/// Assertion helper: how many records are still undelivered.
#[cfg(test)]
pub(super) fn pending_count(connection: &Connection) -> AoneResult<i64> {
    let count = connection.query_row(
        "SELECT COUNT(*) FROM outbox WHERE delivered_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// Records a workspace change inside the caller's transaction, so the
/// notification commits with the facts it describes or not at all. The payload
/// carries only a count: consumers re-read the store for detail rather than
/// trusting a denormalised copy inside an event.
pub(super) fn record_change(
    transaction: &Transaction<'_>,
    kind: &str,
    affected: usize,
) -> AoneResult<()> {
    let payload = serde_json::json!({ "affected": affected }).to_string();
    enqueue(transaction, kind, &payload)?;
    Ok(())
}

fn validate(kind: &str, payload: &str) -> AoneResult<()> {
    if kind.trim().is_empty() || kind.len() > 64 || kind.chars().any(char::is_control) {
        return Err(AoneError::InvalidRequest(
            "outbox event kind must be short, non-empty, and control-free".into(),
        ));
    }
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(AoneError::InvalidRequest(format!(
            "outbox payload exceeds {MAX_PAYLOAD_BYTES} bytes"
        )));
    }
    Ok(())
}

fn now() -> String {
    crate::runner::timestamp::utc_timestamp()
}

#[cfg(test)]
#[path = "outbox_tests.rs"]
mod tests;
