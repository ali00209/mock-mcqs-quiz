# Design

Visual system for MockTest: a printed technical manual for a garage club. Blue
bars, black rules, square corners, condensed-to-wide display capitals over
plain, readable text. The mood is a workshop reference, not a marketing page.

This world was chosen by the user from the "vintage motorcycles" gallery
sample. It replaces the earlier blue-on-grey UI. Product function, copy and
structure are unchanged.

## Mode

Operate. Users take quizzes and check results, so scanning, consistency and
the task come first. Display type is limited to page titles and primary
buttons. Working text, labels and data stay in the normal-width body face.

## Colour

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#e4e6e6` | `#0e1320` | Page background |
| `--surface` | `#f4f5f5` | `#171f31` | Panels, cards, inputs |
| `--ink` | `#15171c` | `#eef1f8` | Text, rules, borders |
| `--muted` | `#565d68` | `#a7b0c4` | Secondary text (5:1 on `--bg`) |
| `--topbar` | `#1f4aa0` | `#1b2d5e` | Header and side tabs |
| `--accent` | `#1f4aa0` | `#6f90e8` | Primary actions, links, current state |
| `--accent-text` | `#ffffff` | `#0b1020` | Text on accent |
| `--ok` / `--bad` / `--warn` | `#1f7a4a` / `#b3261e` / `#8a5a0b` | `#4cc27c` / `#ef6b6b` / `#e0a84f` | Result and state only |

Blue carries action and current location. Black carries structure. Status
colours appear only where a result or warning needs them, and always with text.

## Type

- **Display** (Archivo, width 125%, weight 900, uppercase): page titles,
  primary buttons, the brand, side tabs. Never used for body text or data.
- **Heads** (Archivo, width 112%, weight 800, uppercase, tracking 0.06em):
  section titles inside panels and question titles.
- **Body** (Archivo, normal width, 15px, line height 1.55).
- **Data** (IBM Plex Mono): scores, timers and counts, so digits line up.

Tracking on display capitals stays between -0.02em and 0.04em.

## Shape and rules

- Square corners everywhere, with `--radius: 0`. The reference is a printed
  sheet, not a rounded app, and the user chose that look. This is a deliberate
  exception to the craft floor's 12–16px card-radius guidance, because the
  chosen brief pins the look.
- Panels open with a 2px black rule across the top. Page titles sit under a
  6px rule.
- Borders are 2px ink for controls and cards, 1px for dividers.
- Elevation is a soft shadow only on floating menus. Nothing else casts one.

## Components

- **Top bar:** blue, with the brand in uppercase display type and uppercase
  links. The current page link is underlined.
- **Side tabs:** on screens 720px and wider, black vertical tabs fixed to the
  right edge. The current tab is blue. They are visual repeats of the top links,
  so they are hidden from assistive technology.
- **Buttons:** uppercase display type on primary actions, blue fill with white
  text. Secondary buttons are white with a 2px black border. Hover inverts to
  black. Targets are at least 44px tall.
- **Question navigation:** square 44px chips. The current question is blue, and
  answered questions get a light blue tint.
- **Choices:** bordered rows. A selected choice takes a light blue tint and a
  blue border.
- **Review:** correct and incorrect status is carried by a badge and text,
  never a coloured side border.

## Motion

None. The reference is static, and the app is used for timed work, so there is
no decorative movement. Hover and focus change colour only.

## Imagery and icons

No photographs or illustrations. Icons are drawn in one 2px stroke at the
current text colour. Plain text carries every meaning.

## Anti-references

- Rounded cards with soft shadows (the earlier look).
- Gradients, glass and blur.
- Coloured left-border callouts.
- Display type in body text, labels or data.
