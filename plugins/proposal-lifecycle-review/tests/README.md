# Tests

Helper-script tests are preserved in:

- `skills/proposal-implementation-audit/tests/test_discover_prior_review.py`
- `skills/proposal-implementation-audit/tests/test_report_path.py`

Run them from the plugin root with:

```bash
python3 -m unittest discover -s skills/proposal-implementation-audit/tests
```

The proposal-review-router source package did not include executable tests; its eval scenarios are preserved under `skills/proposal-review-router/evals/scenarios.yaml` and merged into the plugin-level `evals/scenarios.yaml`.
