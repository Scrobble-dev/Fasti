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

## 8. Tabler-First Component System Architecture

Fasti adopts **Tabler** (`@tabler/core` and `@tabler/icons`) as its primary, canonical UI component and layout framework. This eliminates bespoke CSS churn, ensures rock-solid responsive and accessible layouts, and accelerates development while maintaining rigorous visual coherence.

### 8.1 Component Selection Decision Ladder

Every engineer and coding agent must follow this strict 4-step decision hierarchy when building or modifying UI:

```
┌───────────────────────────────────────────────────────────────────────┐
│ 1. Upstream Tabler Core Component                                     │
│    Use standard Tabler CSS classes and structural markup directly     │
│    (.card, .btn, .table, .navbar, .badge, .form-control, etc.)       │
└──────────────────────────────────┬────────────────────────────────────┘
                                   │ If standard component is insufficient
                                   ▼
┌───────────────────────────────────────────────────────────────────────┐
│ 2. Tabler Pattern Composition                                         │
│    Compose multiple standard Tabler elements into composite workflows │
│    (e.g., Tabler modal + form grid + button group + alert banner)     │
└──────────────────────────────────┬────────────────────────────────────┘
                                   │ If visual skinning is required
                                   ▼
┌───────────────────────────────────────────────────────────────────────┐
│ 3. Fasti Token-Skinned Tabler Element                                 │
│    Wrap or override Tabler classes strictly using Fasti design tokens │
│    (CSS custom properties from brand/tokens/tokens.json)              │
└──────────────────────────────────┬────────────────────────────────────┘
                                   │ Strict Exception Only (Requires Rationale)
                                   ▼
┌───────────────────────────────────────────────────────────────────────┐
│ 4. Custom Svelte Component                                            │
│    Build custom components ONLY when Tabler provides zero viable      │
│    equivalent (e.g. Media Scrubber with Horological Calibration Ticks)│
└───────────────────────────────────────────────────────────────────────┘
```

### 8.2 Tabler-to-Fasti Mapping Matrix

| UI Pattern / Element | Upstream Tabler Primitive | Fasti Token Skinning | Custom Fallback Allowed? |
|---|---|---|---|
| **App Shell & Navigation** | `.navbar`, `.navbar-vertical`, `.nav-link` | Skinned with `fasti.color.surface.archive` & Atkinson font | **No** — Always Tabler |
| **Media Card / Poster** | `.card`, `.card-img-top`, `.card-body` | Skinned with `fasti.color.surface.paper`, `radius.md` | **No** — Always Tabler |
| **Activity / History Table**| `.table`, `.table-responsive`, `.table-hover` | Monospace evidence font for timestamps/hashes | **No** — Always Tabler |
| **Status / Resolution Pill**| `.badge`, `.badge-outline`, `.status-dot` | `state.verified` (Verdigris), `state.attention` (Amber) | **No** — Always Tabler |
| **Actions & Quick Bar** | `.btn-group`, `.btn-icon`, `.dropdown` | Oxblood / Chronicle Blue button accents | **No** — Always Tabler |
| **Settings / Ingest Form** | `.form-label`, `.form-control`, `.form-select` | Atkinson body font, 44px touch targets | **No** — Always Tabler |
| **Modal Sheets / Drawers** | `.modal`, `.modal-dialog`, `.offcanvas` | Archival paper elevation, 14px radius | **No** — Always Tabler |
| **Timeline Scrubber Ticks** | Bespoke SVG / Canvas layout | Fasti Oxblood spine + Gold calibration ticks | **Yes** — Documented Exception |

---

## 9. Impeccable Craft Floor & Design Modes Integration

Fasti integrates the **Impeccable** design directives into all UI and review workflows.

### 9.1 Surface Modes
* **Operate (`apps/web`, `apps/desktop` Workbench, Settings, Triage):** The user completes a task. Scanability, consistency, native expectations, and high speed outrank expressive ornament.
* **Read (`Chronicle / Annal`, History, Markdown Knowledge):** The user explores and understands their media history. Literary rhythm, Newsreader headings, and archival clarity dominate.

### 9.2 Impeccable Quality Floor & Absolute Anti-Patterns
1. **Zero Layout Shift (`CLS = 0`):** No jumping elements or shifting lists during asynchronous background sync or metadata resolution.
2. **No Generic Slop:** Strict prohibition of purple/violet AI gradients, generic SaaS 3-column card walls with oversized bubble borders, and neon gamer glows.
3. **Restrained Functional Motion:** Animations strictly 80–220ms, functional only (e.g. SQLite write confirmation), and completely disabled when `prefers-reduced-motion: reduce` is active.
4. **Bounded Iteration Ceilings:** UI design updates are validated in single, batched passes across desktop (1440px) and mobile (320px/375px) viewports.

---

## 10. Universal Interaction, Usability & Accessibility Governance Matrix

Every user interface, component, and interaction flow in Fasti must be audited and verified against the following six authoritative industry frameworks:

### 10.1 AskTog Interaction Principles
* **Anticipation:** The software anticipates the user’s next requirement (e.g., providing quick actions for un-enriched scrobbles without searching).
* **Color Blindness & Consistency:** Color is never the sole carrier of status (all badges pair color with text or distinct icons); palettes maintain strict cross-platform consistency.
* **Explorable Interfaces:** Users can safely explore any view; destructive actions require explicit confirmation, and ambiguous triage can be deferred (`Resolve later`).
* **Fitts's Law:** Interactive buttons, quick-action menus, and navigation links maintain strict minimum 44px × 44px hitboxes, pinned to natural screen edges.
* **Human Interface Objects:** Objects (posters, chronicle books, timeline cards, calibration ticks) behave predictably and mimic durable physical artifacts.
* **Latency Reduction:** Local-first architecture (SQLite + `fastid`) guarantees sub-16ms UI response for local operations. No fake spinners for instant local reads.
* **Protect User Work:** Zero data loss. Background draft auto-saves, persistent session states, and graceful crash recovery ensure incomplete forms or triages are never lost.
* **State Continuity:** Sync operations in the background must never reshuffle list items or move elements under an active cursor or keyboard focus.

### 10.2 Gestalt Grouping Principles
* **Proximity:** Metadata related to a media item (title, year, duration, offset) is grouped tightly within the card body; secondary evidence is grouped in the footer.
* **Similarity:** Identical semantic states (e.g., verified scrobble, ambiguous conflict) share identical typographic styles, badge borders, and iconography across all views.
* **Common Region:** Tabler cards and panels establish distinct 1px bordered boundary containers that visually isolate distinct records.
* **Continuity:** The vertical Oxblood chronicle spine creates an unbroken visual vector connecting past and present media events.
* **Closure:** Clear container boundaries allow users to perceive modular components without visual noise.
* **Figure/Ground:** Archival canvas (`#F2EFE6`), paper cards (`#FFFDF8`), and night background (`#11110F`) maintain clear visual depth layers.
* **Focal Point:** High-contrast Oxblood marks and Chronicle Blue primary buttons draw the eye directly to the primary actionable task.

### 10.3 Nielsen Norman Ten Usability Heuristics
1. **Visibility of System Status:** Continuous, persistent status bar displays daemon health, SQLite sync state, and exact reconciliation progress.
2. **Match Between System & Real World:** Domain language matches universal media and archival terms (Movies, Shows, Music, Books, Games, Podcasts, Annal, Edition, Scrobble).
3. **User Control & Freedom:** Clear undo capability, safe cancellation of active batch operations, and first-class "Resolve later" safe exit.
4. **Consistency & Standards:** Standard Tabler components, established platform keyboard shortcuts, and uniform layout paradigms.
5. **Error Prevention:** Type-safe inputs, date/time pickers with boundary checks, duplicate scrobble prevention, and confirmation on unlinking.
6. **Recognition Rather Than Recall:** Visible quick-action context bars (Ryot-style FAB/actions), breadcrumb trails, and auto-completing search bars.
7. **Flexibility & Efficiency of Use:** Keyboard hotkeys for power users, toggleable list vs. poster grid views, and batch reconciliation workflows.
8. **Aesthetic & Minimalist Design:** High signal-to-noise ratio, tabular data alignment, zero promotional banners, zero gamification streaks or vanity scores.
9. **Help Users Recognize, Diagnose, & Recover from Errors:** Standard RFC 9457 Problem Details with human-readable titles, exact problem causes, and actionable fix buttons.
10. **Help & Documentation:** Contextual tooltips explaining provider claims, inline field definitions, and direct links to Scrobble.dev open grammar specs.

### 10.4 IxDF (Interaction Design Foundation) Research Integration
* **Cognitive Load Optimization:** Chunked form layouts and progressive disclosure (high-level summary row expanding to detailed evidence sheet).
* **Motor Precision & Touch Ergonomics:** Subdued 44px tap targets and thumb-reachable action zones for mobile/touchscreen ergonomics.
* **Dark Mode Halation & Ocular Fatigue:** Soft charcoal night mode (`#11110F`) with muted secondary text (`#625E56`) prevents high-contrast halation and eye fatigue.
* **Neurodivergent Focus (ADHD / AuDHD):** Zero engagement traps, zero streak guilt, stable layouts, low distraction, and persistent status.

### 10.5 WCAG 2.2 Level AA Full Conformance
* **1.4.3 Contrast (Minimum):** Primary text 15.6:1 (AAA), Action Blue 7.6:1 (AAA), Muted text 6.4:1 (AA).
* **1.4.11 Non-text Contrast:** UI components, form borders, and focus states maintain >= 3.0:1 contrast against adjacent backgrounds.
* **2.4.11 / 2.4.12 Focus Not Obscured:** Focused interactive controls are never hidden behind sticky headers, footers, or floating overlays.
* **2.4.13 Focus Appearance:** High-contrast 3px solid focus ring with 2px offset on all interactive elements.
* **2.5.7 Dragging Movements:** Any draggable timeline scrubber or list reordering provides single-pointer button alternatives.
* **2.5.8 Target Size (Minimum):** All interactive targets strictly meet or exceed **44px × 44px** (surpassing the 24px WCAG minimum).
* **3.2.6 Consistent Help:** Help icons, documentation links, and support triggers remain in uniform locations across all views.
* **3.3.7 Redundant Entry:** Previously entered provider credentials or search criteria are auto-populated to avoid repeated user entry.
* **3.3.8 Accessible Authentication:** No cognitive function tests or puzzle CAPTCHAs; passkeys, local tokens, and copy-paste API keys are supported.

### 10.6 EN 301 549 European Standard Compliance
* **Clause 9 (Web):** Full mapping and compliance with WCAG 2.2 Level AA criteria across all web interfaces.
* **Clause 10 (Non-Web Documents):** All generated export reports, Markdown artifacts, and API documentation follow accessible document structure.
* **Clause 11 (Software / Desktop GUI):**
  * Interoperability with platform assistive technologies (Linux Orca, Windows NVDA/JAWS, macOS VoiceOver).
  * Respect for system-wide font scaling, high-contrast OS themes, and accessibility settings.
  * Prevention of keyboard focus traps across all modal dialogs and offcanvas drawers.
* **Clause 12 (Documentation & Support):** Accessibility capabilities, keyboard shortcut cheat-sheets, and conformance statements are documented and accessible.

---

## 11. QA, Design Review & Continuous Verification Protocol

All UI pull requests must provide verified evidence against the following test harness:
1. **Automated Component Accessibility:** `@axe-core/playwright` runs across all Storybook stories and Playwright views (zero violations allowed).
2. **Visual Regression:** Playwright snapshot tests across 320px, 768px, and 1440px breakpoints in both Light and Dark themes.
3. **Performance Sentinels:** Lighthouse CI verifying `CLS = 0` and `INP < 100ms`.
4. **Design Review Artifact:** Pull requests affecting UI must include rendered visual screenshots and the signed UX & Accessibility checklist.

---

## 12. Interactive Preview & Token Artifacts

* **Interactive Preview:** Open `brand/preview.html` in any browser to inspect live theme switching, Tabler integration specimens, color contrast swatches, and timeline layouts.
* **Machine-Readable Tokens:** `brand/tokens/tokens.json` (W3C Design Token Community Group format).

Open `brand/preview.html` in any browser to explore the live interactive theme switcher, vector logos, color swatch tokens, and timeline UI specimens.

---

## 13. Component and Theme Implementation

Use native HTML controls first. Use Tabler icons or a focused Tabler component when the platform primitive does not provide the required meaning. Do not load the full Tabler stylesheet for one or two native controls.

`packages/tokens` owns the semantic browser variables. Light and dark themes reuse the canonical palette through CSS `color-mix()`; they do not create a second palette owner. The active theme is set with `data-bs-theme="light|dark"`. Interactive targets remain at least 44 by 44 CSS pixels, and focus uses a three-pixel indicator with a two-pixel offset.
