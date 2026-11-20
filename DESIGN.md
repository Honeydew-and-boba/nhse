# NHSE interface

The desktop uses the existing React components and theme tokens. The Switch app uses the Inter software renderer, a 1280×720 frame, and the colors in `crates/nhse-switch/src/theme.rs`.

Switch screens retain the existing list, selection outline, right information panel, and button footer implemented in `ui.rs`. New editors reuse `View`, `Row`, `Nav`, the system keyboard, item search, confirmations, and toast feedback. Terrain and design previews occupy the right panel. Keep controls legible in both light and dark themes.

Edits change the session working copy. The existing backup, explicit save, and discard flows own persistence. Villager replacement requires confirmation. File exports preserve existing paths. Errors retain the current editor and appear through existing error feedback. The Switch interface currently uses English labels and numeric coordinates and style IDs.
