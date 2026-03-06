# GitPython Fixture Imports

These files are copied from `GitPython/test/fixtures` and are used in
`gitgpui-git-gix` parser tests to validate edge cases that also matter for
gitgpui:

- ref names with path components (`for_each_ref_with_path_component`)
- paths containing spaces (`diff_file_with_spaces`)
- paths containing `:` (`diff_file_with_colon`)
- unicode rename paths (`diff_rename`)
- additional raw status kinds (`diff_copied_mode_raw`, `diff_change_in_type_raw`,
  `diff_rename_raw`, `diff_raw_binary`, `diff_index_raw`)
- unsafe/quoted path variants from patch output (`diff_patch_unsafe_paths`)
- uncommon pull-style ref prefixes (`uncommon_branch_prefix_FETCH_HEAD`)
- commit metadata fixtures used for log pretty-format parsing (`rev_list_single`,
  `rev_list_commit_stats`)
- `git blame --line-porcelain` parsing (`blame`, `blame_complex_revision`,
  `blame_binary`)
