<!--
Provenance: synthetic regression case for Fasti issue #48. The event identity
contains 257 ASCII bytes, one beyond the public IntegrationObservationRequest
bound of 256 bytes. It models an unbounded or attacker-controlled provider
identifier without copying data from a real account.
-->

# Oversized source event identity

The body is valid JSON, but source_event_id is one byte over its 256-byte
contract limit. The webhook must return a typed invalid_observation problem
before evidence is persisted.
