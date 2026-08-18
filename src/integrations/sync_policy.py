"""Shared limits for durable third-party synchronization state."""

CURSOR_VERSION = 2
SNAPSHOT_CURSOR_VERSION = 1
CURSOR_MAX_AGE_SECONDS = 30 * 24 * 60 * 60
INTEGRATION_PAGE_MAX = 100
CHANGE_PAGE_MAX = INTEGRATION_PAGE_MAX
SNAPSHOT_PAGE_MAX = INTEGRATION_PAGE_MAX

# Change events outlive every valid cursor. This gives clients a bounded
# offline-reconnect window without keeping append-only synchronization state
# forever. Expired clients recover through checkpoint + snapshot + delta.
CHANGE_RETENTION_DAYS = 45

# Completed delivery receipts only need to outlive the supported retry and
# cursor recovery window. Incomplete receipts are preserved separately because
# their mutation outcome is uncertain and requires reconciliation.
COMPLETED_RECEIPT_RETENTION_DAYS = 45

# Keep maintenance batches portable across SQLite and PostgreSQL and small
# enough to avoid long write locks in packaged or low-resource deployments.
DEFAULT_PRUNE_BATCH_SIZE = 500
MAX_PRUNE_BATCH_SIZE = 500

INCOMPLETE_RECEIPT_STATUS = 0
