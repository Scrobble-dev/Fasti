<!--
Provenance: synthetic regression case for Fasti issue #48. The title field is
an object instead of the contract's optional string, modelling a provider
schema drift or nested attacker payload. It contains no live provider data.
-->

# Unexpected nested title

The strict integration DTO must reject this type mismatch as a typed
malformed_json problem and must not panic or enter the durable acceptance
path.
