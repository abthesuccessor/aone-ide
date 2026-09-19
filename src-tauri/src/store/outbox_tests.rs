use rusqlite::Connection;

use super::{
    MAX_PAYLOAD_BYTES, MAX_RETAINED_DELIVERED, SCHEMA, enqueue, mark_delivered, pending,
    pending_count,
};

fn connection() -> Connection {
    let connection = Connection::open_in_memory().expect("in-memory database");
    connection.execute_batch(SCHEMA).expect("outbox schema");
    connection
}

#[test]
fn a_rolled_back_transaction_records_no_event() {
    let mut connection = connection();
    let transaction = connection.transaction().unwrap();
    enqueue(&transaction, "graph.replaced", r#"{"files":3}"#).unwrap();
    // The whole point of the pattern: the event cannot outlive its transaction.
    transaction.rollback().unwrap();
    assert_eq!(pending_count(&connection).unwrap(), 0);
    assert!(pending(&connection, 10).unwrap().is_empty());
}

#[test]
fn a_committed_transaction_records_its_event_exactly_once() {
    let mut connection = connection();
    let transaction = connection.transaction().unwrap();
    enqueue(&transaction, "graph.replaced", r#"{"files":3}"#).unwrap();
    transaction.commit().unwrap();

    let records = pending(&connection, 10).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, "graph.replaced");
    assert_eq!(records[0].payload, r#"{"files":3}"#);
    assert!(records[0].recorded_at.ends_with('Z'));
}

#[test]
fn records_drain_in_commit_order_and_stay_pending_until_acknowledged() {
    let mut connection = connection();
    for index in 0..5 {
        let transaction = connection.transaction().unwrap();
        enqueue(
            &transaction,
            "graph.replaced",
            &format!("{{\"n\":{index}}}"),
        )
        .unwrap();
        transaction.commit().unwrap();
    }

    let first = pending(&connection, 3).unwrap();
    assert_eq!(first.len(), 3);
    assert!(
        first
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );

    // Reading does not acknowledge: an interrupted consumer sees them again.
    assert_eq!(pending(&connection, 3).unwrap(), first);

    // Acknowledge only what was handled; the remainder is still pending.
    mark_delivered(&connection, first[1].sequence).unwrap();
    let remaining = pending(&connection, 10).unwrap();
    assert_eq!(remaining.len(), 3);
    assert_eq!(remaining[0].sequence, first[2].sequence);
}

#[test]
fn acknowledging_is_idempotent_and_never_moves_backwards() {
    let mut connection = connection();
    let transaction = connection.transaction().unwrap();
    let sequence = enqueue(&transaction, "graph.replaced", "{}").unwrap();
    transaction.commit().unwrap();

    assert_eq!(mark_delivered(&connection, sequence).unwrap(), 1);
    // Repeating the acknowledgement changes nothing and resurrects nothing.
    assert_eq!(mark_delivered(&connection, sequence).unwrap(), 0);
    assert_eq!(pending_count(&connection).unwrap(), 0);
}

#[test]
fn delivered_history_is_pruned_while_pending_records_are_never_dropped() {
    let mut connection = connection();
    let total = MAX_RETAINED_DELIVERED + 50;
    for _ in 0..total {
        let transaction = connection.transaction().unwrap();
        enqueue(&transaction, "graph.replaced", "{}").unwrap();
        transaction.commit().unwrap();
    }
    let all = pending(&connection, super::MAX_DRAIN_RECORDS).unwrap();
    let highest = all.last().unwrap().sequence;
    mark_delivered(&connection, highest).unwrap();

    let retained: i64 = connection
        .query_row("SELECT COUNT(*) FROM outbox", [], |row| row.get(0))
        .unwrap();
    let still_pending = pending_count(&connection).unwrap();
    assert!(
        retained <= MAX_RETAINED_DELIVERED as i64 + still_pending,
        "delivered history was not pruned: {retained} rows"
    );
    // Everything not acknowledged survived pruning.
    assert_eq!(still_pending, (total - all.len()) as i64);
}

#[test]
fn oversized_and_malformed_events_are_refused_before_they_are_stored() {
    let mut connection = connection();
    let transaction = connection.transaction().unwrap();
    let oversized = "a".repeat(MAX_PAYLOAD_BYTES + 1);
    assert!(enqueue(&transaction, "graph.replaced", &oversized).is_err());
    for kind in ["", "   ", "kind\nwith\nnewlines", &"k".repeat(65)] {
        assert!(
            enqueue(&transaction, kind, "{}").is_err(),
            "accepted {kind:?}"
        );
    }
    transaction.commit().unwrap();
    assert_eq!(pending_count(&connection).unwrap(), 0);
}

#[test]
fn a_drain_is_bounded_however_long_the_backlog_is() {
    let mut connection = connection();
    for _ in 0..(super::MAX_DRAIN_RECORDS + 25) {
        let transaction = connection.transaction().unwrap();
        enqueue(&transaction, "graph.replaced", "{}").unwrap();
        transaction.commit().unwrap();
    }
    assert_eq!(
        pending(&connection, usize::MAX).unwrap().len(),
        super::MAX_DRAIN_RECORDS
    );
}
