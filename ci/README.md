# SoftGPU CI notes

## Jobs

| Job | Meaning of green |
| --- | --- |
| `core-macos` | Format, `cargo test --locked`, Clippy, doc links, profile validate |
| `core-linux` | Same without requiring ROCm |
| `rocm-integration` | **Skipped** in Phase 0 (`if: false`). Skipped ≠ passed. |
| `ci-result-policy` | Reminder job after cores pass |

## Local smoke

```bash
cargo test --locked
bash tools/check-docs-links.sh
```
