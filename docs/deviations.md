# Intended Deviations Registry

Policy: **`plan.md` §16**. Vanilla 1.8.9 is the default everywhere; every *intentional*
change from vanilla is listed here.

The differential harness treats registered entries as **expected** and **any unregistered
difference as a failure**.

## Rules

- **Implementation defects** (crashes, leaks, GC, races, overflow) are fixed freely and are
  **not** listed here — they are invisible.
- **Presentation** fixes are client-side and keep the same art: "same art, no glitches".
- **Gameplay** fixes apply only where our engine is authoritative (our server / singleplayer)
  and must keep **protocol 47** interoperability.
- **Worldgen, redstone, physics**: never deviate (hard constraint).
- **No protocol divergence** from vanilla peers.

## Fields

| Field | Meaning |
|---|---|
| `id` | stable slug, e.g. `presentation-block-seams` |
| `title` | short human name |
| `category` | `presentation` \| `gameplay` \| `stability` \| `security` |
| `scope` | `client` \| `server` \| `both` |
| `default` | `on` \| `off` |
| `description` | what differs from vanilla |
| `rationale` | why it's the superior behavior |
| `risk` | what could break / who might object |
| `test` | harness that proves everything else still matches |
| `status` | `planned` \| `implemented` |

## Entries

_None yet._ (Seed candidates are listed in `plan.md` §16.5.)

| id | title | category | scope | default | test | status |
|---|---|---|---|---|---|---|
