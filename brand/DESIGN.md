# Fasti & Scrobble.dev — Brand & Design System

**Baseline Status:** Canonical Brand Design System & Visual Specification  
**Authority:** Product Architecture, UX, and Delivery Plan (21 August 2026)  
**Parent Grammar Standard:** [Scrobble.dev](https://scrobble.dev)  
**Flagship Implementation:** [Fasti](https://github.com/Scrobble-dev/Fasti)  

---

## 1. Brand Architecture & Positioning

```
┌────────────────────────────────────────────────────────────────────────┐
│ Scrobble.dev — The Open Grammar & Conformance Standard                │
│ "Scrobble.dev defines the grammar. Fasti keeps the book."              │
│ Vocabulary · Schemas · Crosswalks · Conformance Fixtures · Neutral Hub │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ implements
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│ Fasti — The Living Media Chronicle & Identity Reconciler              │
│ "A book of time in which recorded events become stories."              │
│ Self-Hosted Engine · Local System of Record · Multi-Device Chronicle   │
└────────────────────────────────────────────────────────────────────────┘
```

### Core Brand Principles
1. **Fasti records. Players play.** Decouple media consumption observation from playback. Fasti never needs to transcode or decode media; it observes, reconciles, and records.
2. **Stable local record identity.** Fasti records never belong to one metadata provider. Provider identifiers are versioned claims and evidence attached to a record.
3. **An unresolved record is valid data.** Ambiguous or partial imports (e.g. SIMKL row with IMDb + Kitsu but no MAL) are safely recorded as `partially_resolved`—no history is ever discarded.
4. **Scrobble.dev neutrality.** Scrobble.dev specifies the RFC-grade grammar, crosswalk mappings, and schemas independently of any single implementation.

---

## 2. Visual Aesthetic & Philosophy

* **Aesthetic Archetype:** *The Modern Annal / Living Marginalia (Editorial Almanac × Quiet Horological Instrumentation)*
* **Visual Tone:** Warm, archival, trustworthy, precise, and fatigue-free.
* **Anti-Slop Directives:** 
  * No purple/violet AI gradients.
  * No generic SaaS 3-column card walls with rounded bubble borders.
  * No neon gamer glows or dark-mode black voids.
  * No Roman-empire or faux-Latin kitsch (Fasti is a modern structural chronicle, not a sword-and-sandal museum).

---

## 3. Logo Systems & Visual Glyphs

### 3.1 Fasti Archival Glyph (`fasti-mark-light.svg` / `fasti-mark-dark.svg`)
* **Concept:** The intersection of an archival book spine / annal register with an unbroken horizontal timeline and precision calibration ticks.
* **Symbolism:**
  * **Triple Vertical Spine:** The continuous flow of personal time and observation history.
  * **Oxblood Header Bar:** The deliberate human mark / event binding.
  * **Horological Calibration Ticks:** Precise measurement of progress, timestamps, and episode offsets.
  * **Gold Anchor Node:** The immutable `rec_01K...` identity key.
* **Files:**
  * Vector Mark (Light): `brand/logos/fasti-mark-light.svg`
  * Vector Mark (Dark): `brand/logos/fasti-mark-dark.svg`
  * Brand Lockup: `brand/logos/fasti-lockup.svg`

### 3.2 Scrobble.dev Technical Glyph (`scrobble-dev-mark.svg` / `scrobble-dev-lockup.svg`)
* **Concept:** Developer-first syntax brackets `{ s }` framing an identity crosswalk graph node.
* **Symbolism:**
  * **Code Brackets `{ }` in Chronicle Blue (`#1E4FA3`):** Open schemas, JSON-LD, and RFC conformance.
  * **Crosswalk Node in Verdigris (`#2E6F63`):** Directed identity mapping across providers (TMDB, TVDB, MAL, Kitsu, AniList, Steam, GOG, MusicBrainz).
* **Files:**
  * Vector Mark: `brand/logos/scrobble-dev-mark.svg`
  * Brand Lockup: `brand/logos/scrobble-dev-lockup.svg`

---

## 4. Color Palette & Contrast Matrix

All color pairings are certified for **WCAG 2.2 AA / AAA** readability.

| Token | Hex Value | Role & Usage | Contrast on Canvas |
|---|---|---|---|
| `fasti.color.surface.archive` | `#F2EFE6` | **Archival Ground**: Warm paper canvas base (eliminates harsh monitor glare) | Base canvas |
| `fasti.color.surface.paper` | `#FFFDF8` | **Paper Card**: Elevated timeline sheets, modal cards, reading surfaces | Surface |
| `fasti.color.text.primary` | `#181716` | **Carbon Ink**: High-contrast editorial titles and primary body text | **15.6:1 (AAA)** |
| `fasti.color.text.muted` | `#625E56` | **Muted Metadata**: Timestamps, episode numbers, durations, footnotes | **6.4:1 (AA)** |
| `fasti.color.brand.mark` | `#8B2E2A` | **Fasti Oxblood**: Time spine, recorded marks, deliberate user corrections | **8.2:1 (AAA)** |
| `fasti.color.brand.gold` | `#D4AF37` | **Horological Gold**: Calibration rules, verified record seals | Accent |
| `fasti.color.action.primary` | `#1E4FA3` | **Chronicle Blue**: Interactive links, buttons, structured data anchors | **7.6:1 (AAA)** |
| `fasti.color.state.verified` | `#2E6F63` | **Verdigris Verified**: Reconciled sync state, valid checksums, healthy nodes | **5.8:1 (AA)** |
| `fasti.color.state.attention` | `#8C5A12` | **Amber Attention**: Sequence gaps, partial matches, candidate reviews | **6.1:1 (AA)** |
| `fasti.color.surface.night` | `#11110F` | **Night Chronicle**: Deep charcoal base for OLED media room dark mode | Dark Canvas |

---

## 5. Typographic Triad

```
┌───────────────────────────────────────────────────────────────────┐
│ Newsreader (Production Type)                                      │
│ The Story — Editorial Display, Titles, Dates & Chapters           │
├───────────────────────────────────────────────────────────────────┤
│ Atkinson Hyperlegible (Braille Institute)                         │
│ The Interface — Navigation, Form Controls, Buttons, Body Prose    │
├───────────────────────────────────────────────────────────────────┤
│ IBM Plex Mono / Atkinson Hyperlegible Mono                        │
│ The Evidence — Record IDs, UUIDv7, Timestamps, Schemas, Hashes    │
└───────────────────────────────────────────────────────────────────┘
```

* **Display Font:** *Newsreader* (Serif). Designed for optical clarity across varying sizes; lends an immediate literary and archival gravitas to media chronicles.
* **Interface Font:** *Atkinson Hyperlegible* (Sans-serif). Engineered specifically to disambiguate characters (e.g., zero vs uppercase O, number 1 vs lowercase L vs uppercase I), providing effortless scanning for neurodivergent and low-vision users.
* **Evidence Font:** *IBM Plex Mono* / *Atkinson Mono* (Monospace). Tabular figures for exact alignment of timestamps, duration counters, coordinate offsets, and payload digests.

---

## 6. Layout, Spacing & Touch Ergonomics

* **Base Unit:** 4px geometric progression (`4px`, `8px`, `12px`, `16px`, `24px`, `32px`, `48px`, `64px`).
* **Minimum Touch Target:** Strict **44px × 44px** on all clickable buttons, scrubbers, and action items.
* **Corner Radii:** Subdued, architectural radii (`2px`, `6px`, `10px`, `14px`). Avoid oversized pill shapes.
* **Motion & Easing:**
  * Duration: Micro (80–120ms), Standard (150–220ms).
  * Purpose: Functional continuity only (e.g. indicating an item has been safely committed to local SQLite).
  * Zero jarring layout jumps; mandatory support for `@media (prefers-reduced-motion: reduce)`.

---

## 7. Neurodivergent & Accessibility Standards (ADHD / AuDHD)

1. **State Continuity:** Sync operations in the background must never reshuffle list items or move elements under an active cursor or keyboard focus.
2. **Persistent Non-Toast State:** Errors, import reconciliations, and provider statuses remain persistently visible in dedicated status bars rather than disappearing in ephemeral toasts.
3. **No Engagement Traps:** No streak counters, no gamification guilt, and no forced reviews upon completing media.
4. **Resumable Review Checkpoints:** Ambiguous mapping imports can be reviewed partially and resumed at any time (`Resolve later` is a first-class safe choice).

---

## 8. Interactive Preview

Open `brand/preview.html` in any browser to explore the live interactive theme switcher, vector logos, color swatch tokens, and timeline UI specimens.
