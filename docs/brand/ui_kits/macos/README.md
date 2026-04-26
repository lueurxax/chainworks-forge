# Forge — macOS UI kit

Recreation of the Chainworks Forge SwiftUI app, redesigned against Apple HIG
on the existing `Forge*` token primitives.

## Surfaces covered

- **Sidebar** with the seven operator tabs (Runs, Ideas, Approvals, Agent
  catalog, Workflow inspector, Pilot readiness, Settings) and the brand mark.
- **Toolbar** with translucent material (`backdrop-filter: blur(20)
  saturate(180%)`), segmented filter, and primary action.
- **Runs Home** — grouped run list (Active / Recent), `RunRow` with
  status capsule, progress bar tinted by status, and metadata strip.
- **Run detail** — header, approval banner, frozen stage rail (with running
  pulse + state colors), and artifact rows (JSON / Markdown / diff).
- **Inspector** — frozen run snapshot, approval gate panel, recovery panel.
- **Ideas, Approvals, Agent catalog, Workflow inspector, Pilot readiness,
  Settings** — secondary surfaces.

## Files

| File | What's in it |
| --- | --- |
| `index.html` | Mounts the app. |
| `styles.css` | Tokenised CSS — imports `colors_and_type.css` and adds Forge surfaces. |
| `Primitives.jsx` | `Icon`, `Capsule`, `SectionHeader`, `NavItem`, `STATUS_META`. |
| `Shell.jsx` | `Sidebar`, `Toolbar`. |
| `RunsHome.jsx` | `RunRow`, `RunsHome`, run fixtures. |
| `RunDetail.jsx` | Stage rail, artifact rows, approval/blocked banners. |
| `Inspector.jsx` | Right-pane snapshot + approval + recovery. |
| `App.jsx` | App root + secondary tabs. |

## Notes

- Icons are inline Lucide-style SVGs; in production SwiftUI we use SF Symbols
  (see ICONOGRAPHY in the root README).
- Run, stage, and artifact data are static fixtures. The intent is pixel-level
  recreation of the operator surfaces, not a working backend.
