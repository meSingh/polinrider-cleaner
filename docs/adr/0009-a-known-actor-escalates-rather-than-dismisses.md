# 0009. A known actor escalates rather than dismisses

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

The push ledger records who pushed, not whether they should have.

An earlier version had `--trusted-actor`, which discounted a named person's pushes. Run against a real incident it emptied the hostile-push list and turned eight restore candidates into zero.

## Decision

`--known-actor` names someone you recognise. It does **not** discount their pushes. It adds them to a list of machines that need checking, and the plan says why.

## Consequences

This campaign propagates by amending and force-pushing as whoever is logged in. A colleague's name on a hostile push is the expected case, not an exculpatory one: it usually means their machine is compromised.

Discounting by identity therefore deletes exactly the evidence that matters. No code path may do it.

Cleaning repositories does not hold while an infected machine can still push to them, so naming a colleague makes the incident bigger, not smaller.
