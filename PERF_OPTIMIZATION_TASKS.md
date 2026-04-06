# Performance Optimization Tasks

## Incident

- Symptom: opening or diffing an approximately 150 MB JSON file crashed GitComet.
- Status: no stable local reproduction has been written into the performance suite yet.
- Primary goal: reduce the crash to one credible benchmark or one narrowly scoped harness reproduction before normal optimization work starts.
- Primary suspects: large-file syntax preparation, diff row materialization, word-diff/token work, or follow-on search/highlight passes triggered by a very large JSON payload.

## Rebaseline Checklist

- [ ] Fresh run reset. Record the exact user action that crashed the app: plain file open, commit diff, working tree diff, conflict view, preview pane, or another path.
- [ ] Capture the input shape. Record approximate byte size, line count, whether the file is minified or pretty-printed, and whether syntax highlighting was enabled when the crash happened.
- [ ] Capture the failure mode. Record whether this is a Rust panic, OOM kill, abort, watchdog timeout, or UI hang followed by crash, and copy the shortest useful summary into the handoff.
- [ ] Run the existing large-text proxy commands from `perf.md` and record the first command that reproduces a crash, timeout, or suspicious allocation spike.
- [ ] If existing proxy benchmarks do not reproduce the failure credibly, add a narrow JSON-specific perf fixture and benchmark case before doing optimization work.
- [ ] Confirm which stage dominates the failure: syntax preparation, diff plan materialization, inline or split row paging, word-diff/token work, search/highlight follow-on work, or something outside the current suite.
- [ ] After one reproduction is credible, record the first stable timing outliers and allocation outliers below. Do not start broad optimization passes before that point.

## Candidate Existing Benchmarks

Start with these existing cases before introducing new harness code:

- `file_diff_syntax_prepare/file_diff_syntax_prepare_cold`
- `file_diff_syntax_query_stress/nested_long_lines_cold`
- `diff_open_file_inline_first_window/200`
- `diff_open_file_split_first_window/200`

Use the size overrides from `perf.md` so the proxies are closer to the original 150 MB incident.

## Notes For The First Optimize Pass

Once a credible reproduction exists, the first optimize task should stay narrow:

- prefer reducing repeated full-document syntax work before touching UI polish or layout paths
- keep the same benchmark label and the same environment variables for before and after measurements
- if a proxy benchmark is not representative enough, replace it with a JSON-specific benchmark instead of optimizing against the wrong workload

## Recorded Timing Outliers

No confirmed timing outliers yet.

## Recorded Allocation Outliers

No confirmed allocation outliers yet.
