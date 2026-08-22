//! Storage adapter boundary.
//!
//! B1 owns application contracts, not persistence. B2 adds the bounded SQLite
//! writer, migrations, durability, and receipt replay. Keeping this crate
//! intentionally empty prevents a scaffold store from being mistaken for a
//! durable implementation.
