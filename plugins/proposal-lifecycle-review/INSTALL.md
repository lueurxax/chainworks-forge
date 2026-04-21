# Install

This is a local Codex plugin package.

## Local Repo Install

The plugin is already laid out as:

```text
plugins/proposal-lifecycle-review/.codex-plugin/plugin.json
```

To use it from a local marketplace, add a marketplace entry like:

```json
{
  "name": "proposal-lifecycle-review",
  "source": {
    "source": "local",
    "path": "./plugins/proposal-lifecycle-review"
  },
  "policy": {
    "installation": "AVAILABLE",
    "authentication": "ON_INSTALL"
  },
  "category": "Productivity"
}
```

The plugin does not require MCP servers or apps.

## Skills Exposed

- `proposal-review-router`
- `proposal-implementation-audit`
- `proposal-lifecycle-review`

## Recommended Repo Templates

Copy only the templates you need:

- `assets/templates/AGENTS-root-template.md` -> repo root guidance
- `assets/templates/AGENTS-proposals-template.md` -> proposal directory guidance
- `assets/templates/proposal-lifecycle-router.yaml` -> `.codex/proposal-lifecycle-router.yaml`
- `assets/templates/review-router.yaml` -> `.codex/review-router.yaml`
- `assets/templates/implementation-audit-router.yaml` -> `.codex/implementation-audit-router.yaml`
- `assets/templates/reviewer-example.yaml` -> `.codex/reviewers/<name>.yaml`
- `assets/templates/implementation-reviewer-example.yaml` -> `.codex/implementation-reviewers/<name>.yaml`

## Validation After Install

From the plugin root:

```bash
python3 -m json.tool .codex-plugin/plugin.json >/dev/null
python3 /Users/user/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/proposal-review-router
python3 /Users/user/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/proposal-implementation-audit
python3 /Users/user/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/proposal-lifecycle-review
python3 -m unittest discover -s skills/proposal-implementation-audit/tests
```
