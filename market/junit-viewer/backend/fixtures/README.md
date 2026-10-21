# JUnit compatibility fixtures

These snapshots were generated locally from disposable test projects on
2026-10-21. They contain intentionally failing tests; they are data fixtures,
not tests run by ToolForge.

- `pytest.xml`: pytest 9.1.1, default xUnit2 family, generated with
  `python -m pytest test_sample.py --junitxml=pytest.xml -o junit_logging=all`.
  Includes one passing test, two parameterized invocations (one failing),
  and one explicitly skipped test. Captured stdout and failure text are kept; hostname is normalized.
- `surefire.xml`: Maven Surefire 3.5.4, JUnit 4.13.2, JDK 21, generated with
  `mvn -Dmaven.test.failure.ignore=true test`. Includes passing, assertion
  failure, thrown exception, and ignored tests. The environment-specific
  `<properties>` body was removed, and temporary absolute paths were replaced
  with `/fixture`. Outcomes, duration fields, CDATA and stack traces are kept.

Sources describing these dialects:

- https://docs.pytest.org/en/stable/how-to/output.html#creating-junitxml-format-files
- https://maven.apache.org/surefire/maven-surefire-plugin/xsd/surefire-test-report.xsd

These fixtures establish compatibility with their generators' output. They
do not claim all producers share one strict JUnit XML schema. Other outcome
extensions are surfaced as diagnostics and, for affected cases, unknown status.
