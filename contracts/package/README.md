# Package-owned action/effect contract

These schemas pin the `vqro.collections` component boundary implemented by this
package. They are not host Collections APIs and grant no execution authority.
The host may validate and execute a returned plan only under its own current
authority fence.

The service method is `host.document.action.invoke`. Its strict params use
`vqro.collections.document-action.v1`; results use `vqro.effect-plan.v1`.
Actions are exactly `vqro.collections.label.set`,
`vqro.collections.terminal.archive`, and
`vqro.collections.terminal.unarchive`. Every successful plan repeats the exact
observed dependency vector, authority generation, namespace, and state revision
and contains one ordered whole-value `state.cas` effect. The state store ID and
generation derive and exactly match the observed `host.state.v1` dependency;
the CAS carries store generation, revision, complete expected value (`null`
means absent), and complete replacement policy value. The component never
executes the plan.

The pinned host `host.document.v2` contract has no action/reference field.
Consequently this candidate does not add action fields to that document or
invent a host attachment mechanism. Existing `terminal_slot` nodes remain
host-owned generic terminal references; this package defines no focus,
navigation, terminal-placement, or terminal-lifecycle action. Integration must
supply a generic host action/reference attachment contract before actions can
be surfaced from the rendered document.

`collections-migration-v1.schema.json` and the paired migration fixtures pin the
pure source-to-plan adapter. This is a package library adapter, not a host
service method; the package does not invent a migration host API or execute the
returned marker/value material.
