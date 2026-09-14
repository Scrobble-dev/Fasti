<!--
Provenance: synthetic regression case for Fasti issue #48. The timestamp uses
the shape of an RFC 3339 value but contains impossible month, day, hour, and
minute components. No provider response or account data is included.
-->

# Malformed observed timestamp

The body is syntactically valid JSON, but observed_at cannot be parsed as an
RFC 3339 instant. The webhook must return a typed invalid_observation problem
before durable acceptance.
