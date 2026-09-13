# Diagnostics (Phase 0)

Phase 0 diagnostics are CLI stderr lines of the form:

```text
error[<category>]: <message>; remediation: <hint>
```

Categories: `config`, `profile`, `unsupported`, `validation`, `internal`, `io`.

Structured JSON Lines tracing, handle IDs, packet/PC context, and sanitizer findings arrive with later phases. Logging must remain bounded and must not leak secrets by default once traces exist.
