# `vqro.collections.policy` version 1 source profile

Status: **package-owned-source-profile-not-host-contract**.

This profile is owned by the external `vqro.collections` package. It neither
asserts that a host writes these values nor authorizes the package to read or
change them. It defines no host call, capability, dependency, migration,
activation, persistence, document acceptance, or rendering contract. Runtime
use remains blocked until a public generic composite state-and-terminal host
gate exists.

## Namespace and key

The expected generic state namespace is `vqro.collections`. Naming it here does
not grant access or ownership.

A policy key is exactly `container_` followed by 16 lowercase hexadecimal
digits. The decoded unsigned 64-bit value must be neither zero nor
`18446744073709551615`; therefore the all-zero and all-`f` forms are invalid.
The key is also the canonical `container_id` stored in its value.

Keys that are not canonical container keys are unrelated package-namespace
values and have no policy meaning. A canonical container key with a malformed
value makes the complete policy projection invalid; it is never treated as a
missing/default record.

## Value

A value is a strict JSON object with no unknown fields:

```json
{
  "archived_terminal_ids": [
    "term_00000000000000000000000000000001"
  ],
  "container_id": "container_000000000000002a",
  "label": "Helpers",
  "schema": "vqro.collections.policy",
  "schema_version": 1
}
```

`schema`, `schema_version`, `container_id`, and `archived_terminal_ids` are
required. `label` is optional; omission means no label, an empty string is a
label, and explicit JSON `null` is invalid. A terminal identity is exactly
`term_` plus 32 lowercase hexadecimal digits.

`archived_terminal_ids` is required even when empty and must contain at most 64
identities in strictly increasing bytewise order without duplicates. A
constructor sorts its input before encoding, but a decoder rejects an unsorted
or duplicate wire value. Unknown schema names and every version other than
integer `1` are rejected. The value's `container_id` must exactly equal its
state key.

Canonical independent encoding is compact UTF-8 JSON with recursively
lexicographically sorted object keys and preserved array order. Optional
`label` is omitted rather than encoded as `null`. Decoding and canonical
re-encoding must produce the same JSON value; malformed inputs are not repaired.

## Bounds

The normative package checks are:

- label: at most 4,096 UTF-8 bytes;
- archived terminal identities per record: at most 64;
- neutral terminal identities reconciled per container: at most 64;
- compact encoded value: at most 32 KiB;
- complete supplied generic namespace map: at most 512 keys and 128 KiB when
  compactly encoded.

The schema's `maxLength` is only a safe structural precheck. The UTF-8 byte
limit and the semantic constraints that JSON Schema cannot express are checked
by the package parser. These limits do not promise that 512 maximum-sized
records fit simultaneously.

## Reconciliation and stale references

Neutral container identity, membership, order, and selection come only from an
independently trusted terminal snapshot. A policy record owns only its optional
label and archive flags. It never supplies pane, workspace, tab, placement,
selection, membership, or ordering authority.

For a current container, reconciliation preserves the supplied neutral terminal
order and marks a terminal archived exactly when its stable identity occurs in
the record. A missing record projects no label and all current terminals as
unarchived. Archived identities absent from current neutral membership are
valid stale references: they remain in the decoded record and are ignored by
the effective projection. Reconciliation never prunes or mutates policy.

The package-owned membership/archive fingerprint vectors use SHA-256 over:

1. ASCII domain `vqro.collections.membership-archive.v1` followed by NUL;
2. container numeric identity as big-endian `u64`;
3. neutral member count as big-endian `u64`;
4. for each neutral terminal in order: byte length as big-endian `u64`, UTF-8
   identity bytes, and one byte (`0` or `1`) for its effective archive flag.

Labels, selection, placement, stale archive references, and archive storage
order are excluded from this fingerprint. This fingerprint is a package test
vector only; it is not a host document dependency or freshness claim.

## Malformed behavior

Invalid key/value identity, unknown fields, missing fields, explicit null label,
wrong schema/version, invalid identities, excessive bounds, noncanonical archive
order, duplicates, and key/value mismatch reject the complete supplied policy
projection. Unknown future versions fail closed. No malformed canonical record
falls back to unlabelled/unarchived behavior.
