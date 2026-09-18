# Legacy Collections migration adapter

The pure adapter accepts the exact strict legacy snapshot contract
`vqro.collections.legacy.v1`: `source_generation` plus a `collections` object
keyed by canonical container IDs. Each value contains required sorted
`archived_terminal_ids` and optional non-null `label`. No unknown fields are
accepted.

It returns `vqro.collections.migration.v1`, namespace `vqro.collections`, the
SHA-256 of the exact source bytes, the source generation, complete policy
values, and marker material at `migration:legacy-v1`. The marker repeats schema,
source digest, and generation so a host-owned rollback/roll-forward journal can
CAS it alongside migrated values. The adapter performs no writes. Repeating it
for identical bytes is byte-for-byte deterministic. This fixture pins the
legacy shape available to this package; any different host legacy shape is a
contract mismatch and must be supplied rather than inferred.
