# Design QA

Status: **Passed**

Date: 2026-07-13

## Verified

- Local preview loads successfully at `620 × 360` in the in-app browser.
- Status banner is visible immediately after data load and automatically hides after 3 seconds.
- Re-rendering the countdown does not restart the banner timer for the same status message.
- Reset countdown uses localized day/hour/minute units and omits zero-valued units.
- Verified Chinese preview output: `3小时 12分` (no `0天` prefix).
- Frontend production build completes successfully.
- Rust provider mapping tests pass (3/3).
- macOS `.app` and `.dmg` bundles build successfully.

## Notes

- Native Windows packaging and native multi-display interaction were not run on this Mac.
- The local-account provider continues to show unavailable fields when the upstream account response omits a quota window.
