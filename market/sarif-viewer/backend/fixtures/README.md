# Fixtures

Static inputs for the test suite. `eslint.sarif` covers the acceptance list of
issue #365: two rules with and without a default level, `ruleIndex` agreement,
artifact indexes, message template interpolation, a baseline state, a
suppression record, related locations and one inlined code flow.
`broken.sarif` is truncated JSON and must fail with `sarif.json`.

New files here must keep the total repository size small; prefer inline JSON in
tests for one-off shapes.
