# Stage 15.17 — Test Plan

> **Date**: 2026-07-31  
> **Version**: v0.142.0 → v0.143.0

## Test cases (9 new unit tests)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_17_color_config_default` | Default is Auto |
| 2 | `stage15_17_colorize_always` | Always → ANSI codes |
| 3 | `stage15_17_colorize_never` | Never → plain text |
| 4 | `stage15_17_colorize_auto` | Auto → plain text (caller resolves) |
| 5 | `stage15_17_level_color_mapping` | Level → Color mapping |
| 6 | `stage15_17_format_snippet_colored_never` | Snippet without color |
| 7 | `stage15_17_format_snippet_colored_always` | Snippet with ANSI ^^^ |
| 8 | `stage15_17_format_with_source_colored_never` | Full display without color |
| 9 | `stage15_17_format_with_source_colored_always` | Full display with color |
