# Fasti Design System & Brand Guidelines

**Aesthetic Archetype:** *The Modern Annal / Living Marginalia (Editorial Almanac × Quiet Instrumentation)*  
**Primary Brand Line:** *"Every story, kept in time."*  
**Product Positioning:** *A self-hosted-first media chronicle and player for what you watch, read, hear and play.*

---

## 1. Visual Philosophy

Fasti does not treat media software as a dark-mode poster wall with flashy gradients and neon glows. Fasti treats personal media activity as a **living chronicle**—an ordered record of time spent with stories, ideas, music, and games.

The interface draws inspiration from historical registers, editorial publications, marginal annotations, and precise physical instrumentation.

### Safe Foundations vs. Creative Risks
* **Safe Category Literacy:** Predictable, accessible navigation; clear media card semantics; standard playback shortcuts; highly legible contrast ratios.
* **Deliberate Creative Risks:** Warm archival background palette (`#F2EFE6` / `#FFFDF8`); serif display typography (*Newsreader*) for titles and dates; literal evidence notation (`occurred_at`, `observed_at`, source provenance) embedded directly into timeline views.

---

## 2. Color System

Fasti’s palette is semantic, archival, and restrained. No interface state relies on color alone.

| Token | Hex Value | Semantic Purpose | Contrast Ratio on Paper |
|---|---|---|---|
| `fasti.color.surface.archive` | `#F2EFE6` | Primary warm ground / background | — |
| `fasti.color.surface.paper` | `#FFFDF8` | Reading cards, modal sheets, timeline rows | — |
| `fasti.color.text.primary` | `#181716` | Main body text, headings, titles | ~15.6:1 (AAA) |
| `fasti.color.text.muted` | `#625E56` | Secondary metadata, dates, durations, captions | ~6.4:1 (AA) |
| `fasti.color.brand.mark` | `#8B2E2A` | **Fasti Oxblood**: Time spine, recorded marks, deliberate human edits | ~8.2:1 (AAA) |
| `fasti.color.action.primary` | `#1E4FA3` | **Chronicle Blue**: Interactive links, buttons, structured data | ~7.6:1 (AAA) |
| `fasti.color.state.verified` | `#2E6F63` | **Verdigris**: Confirmed sync state, validated checksums, green badges | ~5.8:1 (AA) |
| `fasti.color.state.attention` | `#8C5A12` | **Amber**: Sequence gaps, pending retries, conflicts needing review | ~6.1:1 (AA) |
| `fasti.color.surface.night` | `#11110F` | Dark mode base surface, terminal blocks, player shell | — |

---

## 3. Typography Hierarchy

```
┌────────────────────────────────────────────────────────┐
│ Newsreader                                             │
│ The Story — Large editorial titles, dates, chapters    │
├────────────────────────────────────────────────────────┤
│ Atkinson Hyperlegible Next                             │
│ The Interface — Navigation, body prose, card titles    │
├────────────────────────────────────────────────────────┤
│ Atkinson Hyperlegible Mono / IBM Plex Mono             │
│ The Evidence — Event IDs, timestamps, receipts, code   │
└────────────────────────────────────────────────────────┘
```

* **Display / Editorial:** *Newsreader* (Serif). Designed for continuous editorial reading.
* **Body / Product UI:** *Atkinson Hyperlegible Next* (Sans-serif). Engineered by the Braille Institute for maximum character differentiation and readability.
* **Metadata & Evidence:** *Atkinson Hyperlegible Mono* / *IBM Plex Mono* (Monospace). Tabular figures for timestamps, timecodes, progress seconds, and UUIDv7s.

---

## 4. Spacing, Radii & Interaction Standards

* **Base Unit:** 4px grid rhythm (`4px`, `8px`, `12px`, `16px`, `24px`, `32px`, `48px`, `64px`).
* **Touch Targets:** Strict **44px minimum touch target** across all interactive elements (buttons, scrubbers, list selectors) to ensure motor accessibility.
* **Corner Radii:** Restrained architectural radii (`2px`, `6px`, `10px`). Avoid bubbly, hyper-rounded pills.
* **Motion & Transitions:** 
  * Subtle and functional (80–180ms).
  * Used only to explain continuity (e.g. committing a local event to disk, sync state badge updating).
  * Strict support for `prefers-reduced-motion: reduce`.

---

## 5. Cognitive & Neurodivergent Accessibility (ADHD / AuDHD)

1. **State Continuity & Predictability:** Background synchronisation must never unexpectedly shift or reorder list items under active user focus or cursor.
2. **Stable Navigation:** Primary navigation anchors (`Continue`, `Chronicle`, `Library`, `Search`) remain fixed in position.
3. **No Engagement Traps:** Zero streak shame, zero guilt mechanics for inactive periods, and no forced rating prompts upon completing a media item.
4. **Resumable Everything:** Any import, setting, or edit in progress is autosaved locally across tab closures or system restarts.
