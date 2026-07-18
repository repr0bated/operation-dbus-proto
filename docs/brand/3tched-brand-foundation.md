# 3tched — Brand Foundation

**Direction:** The evidence plane of the operation-dbus codebase. 3tched (pronounced "etched") is the company and enterprise-governance brand: schema-driven infrastructure that turns live system state into continuous, verifiable compliance evidence.

**Sibling brand:** GhostBridge (the privacy plane, same codebase). One-line relationship: *GhostBridge hides you from adversaries; 3tched proves you to auditors.* Same schema-driven control plane underneath.

---

## Brand Purpose

Compliance today is reconstruction — screenshots, spreadsheets, and attestations assembled after the fact to describe a system that has already changed. 3tched exists to make the truth about systems durable at the moment it happens: evidence as a byproduct of operation, carved into an append-only record, rather than a quarterly archaeology project.

## Brand Vision

Audits become reads, not projects. Regulated systems carry their own proof, regulators and operators consult the same live record, and "we believe this is configured correctly" is replaced everywhere by "here is the hash, verify it yourself."

## Brand Mission

Deliver a schema-driven governance platform for regulated operators — healthcare, finance, AI systems under the EU AI Act — in which one schema simultaneously defines configuration, enforcement, and the audit record. What runs, what's enforced, and what's reported are the same object, mapped to OSCAL and anchored in a hash chain, so attestation and reality cannot drift apart.

## Brand Values

1. **Immutability**: The record cannot be quietly revised. *Manifestation:* evidence is append-only and hash-anchored; corrections are new entries that reference what they correct. 3tched never offers an "edit history" feature for the record itself — that would be the product contradicting its own name.

2. **One source of truth**: The schema is the system. *Manifestation:* no translation layer between what runs and what's reported — the same schema drives runtime state, enforcement policy, and the compliance artifact. If a control isn't in the schema, 3tched doesn't claim it. Manual evidence assembly is treated as a bug, not a workflow.

3. **Verifiability over trust**: Every claim resolves to something checkable. *Manifestation:* reports link each assertion to a hash boundary an auditor can independently verify; sales and marketing claims follow the same rule. "Trust us" never appears in 3tched materials — the entire pitch is that you don't have to.

## Brand Personality

- **Exacting**: The engraver's temperament — measure precisely, cut once. Expression: numbers carry their provenance, mappings cite the exact OSCAL control ID, nothing is rounded for effect.
- **Calm under scrutiny**: Audit season is the brand's home turf, not its stress test. Expression: unhurried, steady language even in incident and finding contexts; 3tched never sounds defensive because the record speaks first.
- **Transparent**: The brand behaves the way it asks systems to behave. Expression: public documentation of what 3tched does and doesn't cover, published limitations, straightforward pricing. Openness as a demonstration of the product thesis.

## Brand Promise

If it ran, it's on the record. If it's on the record, you can verify it — and so can your auditor, without taking anyone's word for anything, including ours.

---

# Visual Identity System

**Design logic:** The material world of 3tched is inscription — bronze plaques, engraved plates, stone-cut records. The palette is cool limestone and graphite ink with a single verdigris accent: oxidized bronze, the color a permanent record turns as it ages in public. Deliberately cold and mineral, not warm parchment — this is an institution's surface, not a stationery set. The type system is shared with GhostBridge (one IBM Plex superfamily, two expressions — the same codebase wearing two faces): here the *serif* cut steps forward as the display voice, set with generous tracking and small-caps eyebrows for an inscriptional register, while the mono cut is reserved for what it's genuinely for — hashes and evidence, rendered as a visible brand motif.

```css
/* 3tched Design System Variables */
:root {
  /* Primary Brand Colors */
  --brand-primary: #1D1C19;      /* Graphite ink — text, marks, dark surfaces */
  --brand-secondary: #7A766C;    /* Weathered stone — structure, rules, secondary UI */
  --brand-accent: #2F6E62;       /* Verdigris — the patina of a permanent record */

  /* Brand Color Variations */
  --brand-primary-light: #3B3934;
  --brand-primary-dark: #0F0E0C;
  --brand-secondary-light: #A7A399;
  --brand-secondary-dark: #4A4740;

  /* Neutral Brand Palette */
  --brand-neutral-100: #EFEEE8;  /* Limestone */
  --brand-neutral-500: #8F8B82;  /* Stone dust */
  --brand-neutral-900: #14130F;  /* Carbon */

  /* Brand Typography — slot semantics: primary=body/UI, secondary=display, accent=evidence */
  --brand-font-primary: 'IBM Plex Sans', -apple-system, 'Segoe UI', sans-serif;
  --brand-font-secondary: 'IBM Plex Serif', Georgia, serif;         /* display: +0.01em tracking; small-caps eyebrows */
  --brand-font-accent: 'IBM Plex Mono', 'SFMono-Regular', Menlo, monospace;  /* hashes, control IDs, evidence excerpts */

  /* Brand Spacing System */
  --brand-space-xs: 0.25rem;
  --brand-space-sm: 0.5rem;
  --brand-space-md: 1rem;
  --brand-space-lg: 2rem;
  --brand-space-xl: 4rem;
}
```

**Accessibility notes:** Verdigris (#2F6E62) on limestone (#EFEEE8) clears WCAG AA for text at any size (contrast ≈ 5.4:1), so the accent may carry links and control IDs, not just decoration. Body text is graphite on limestone (≈ 15:1). On dark surfaces, lift the accent toward #4E9486 for legibility.

```css
/* Logo Implementation */
.brand-logo {
  min-width: 120px;
  min-height: 40px;
  padding: var(--brand-space-sm);
}

.brand-logo--horizontal {
  /* Wordmark "3tched" in Plex Serif, the numeral 3 cut with a flat
     chiseled terminal so it reads as both "3" and an engraved "E".
     Clearspace: cap height on all sides. */
}

.brand-logo--stacked {
  /* Icon above wordmark; use where width < 200px. */
}

.brand-logo--icon {
  /* The triple stroke: three horizontal chisel cuts (≡), the
     mathematical sign for identity/congruence — the mark literally
     means "provably the same." Each stroke carries a subtle beveled
     edge on light surfaces; flat single-color on dark or small sizes.
     Verdigris on limestone; limestone on graphite. */
  width: 40px;
  height: 40px;
}
```

**Signature visual element:** the evidence strip. Real (truncated) hash fragments set in Plex Mono appear as a compositional element — in report footers, section dividers, and marketing surfaces — always drawn from actual records, never decorative lorem-hashes. The brand shows its receipts even in its typography.

---

# Brand Voice and Messaging

## Voice Characteristics

- **Exacting**: Statements carry their scope. Use in all writing: "maps 47 controls in NIST 800-53 rev 5" rather than "broad framework coverage."
- **Plainspoken**: Readable by a regulator and an engineer in the same sitting. Use in reports and docs: define every acronym once, translate jargon at first contact, never hide behind it.
- **Assured**: The confidence of someone holding the record. Use everywhere: no hedging filler, no defensive qualifiers — limits are stated as facts, not apologies.

## Tone Variations

- **Professional**: Audit deliverables, compliance documentation, regulator-facing material. Formal, citation-ready, control IDs inline. *"Control AC-2 is enforced at the schema layer; the evidence chain for this reporting period resolves to the anchor below."*
- **Conversational**: Sales, onboarding, founder communications. Clear and direct, jargon translated. *"Your auditor asks for proof; today you build a binder. 3tched means the binder already exists, and it can't have been edited."*
- **Supportive**: Audit-season support, findings response. Steady and evidence-first. *"The finding is addressable. Here's what the record shows, and here's the remediation entry that will reference it."*

## Messaging Architecture

- **Brand Tagline**: *Etched, not asserted.*
  (Alternates: *State you can prove.* / *Evidence by architecture.*)
- **Value Proposition**: 3tched turns live system state into continuous, hash-anchored compliance evidence. One schema drives configuration, enforcement, and the audit record — so what you attest is what actually runs, and anyone can verify it.
- **Key Messages**:
  1. *(Compliance & GRC teams)* — Evidence is generated by the system as it operates, already mapped to OSCAL and the frameworks you answer to — NIST, HIPAA, GDPR, the EU AI Act. Audit prep becomes review, not reconstruction.
  2. *(CISOs & platform engineering)* — Configuration, enforcement, and reporting are one schema, so drift between what runs and what's reported is structurally impossible, not procedurally discouraged.
  3. *(Auditors & regulators)* — A verifiable, hash-chained record replaces screenshots and attestations. Every claim resolves to something you can independently check.

## Writing Guidelines

- **Vocabulary**: Prefer *evidence, record, verifiable, attest, anchor, control, schema*. Reserve *etch/carve* for taglines and display copy — once per surface, or it becomes wallpaper. Avoid *audit-proof, guaranteed compliance, effortless, magic* — nothing is audit-proof, and claiming so is disqualifying in this market.
- **Grammar**: Sentence case for headings; small caps reserved for eyebrows and control-family labels. Framework names and control IDs always exact (NIST SP 800-53 rev 5, not "NIST standards"). Present tense for shipped capability; roadmap items are explicitly dated or absent.
- **Cultural Considerations**: Audiences span engineers, compliance officers, and regulators across jurisdictions — plain language is the bridge. Never characterize regulators as adversaries; in 3tched's world the auditor is a user. Avoid US-centric framing when discussing GDPR and EU AI Act material.

---

# Implementation Notes

- Token source of truth: keep this file's `:root` block as `brand/3tched.tokens.css` alongside `brand/ghostbridge.tokens.css` in the operation-dbus repo; both brands, one repo — the file layout restates the positioning.
- Trademark: "3tched" is a coined term with strong distinctiveness — the numeral spelling is an asset for registration. Knockout-search "etched" collisions in Class 9/42 anyway; the ≡ icon is separately registrable.
- Brand architecture: 3tched LLC operates as the house brand (contracts, enterprise sales, NVIDIA Inception application); GhostBridge sits under it as a product brand. Pitch materials can carry both: 3tched signs the letterhead, GhostBridge names the stack.
