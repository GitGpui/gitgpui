use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gitgpui_ui_gpui::benchmarks::{
    CommitDetailsFixture, ConflictResolvedOutputGutterScrollFixture,
    ConflictSearchQueryUpdateFixture, ConflictSplitResizeStepFixture,
    ConflictThreeWayScrollFixture, ConflictTwoWaySplitScrollFixture, LargeFileDiffScrollFixture,
    OpenRepoFixture,
};
use std::env;
use std::time::Duration;

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn bench_open_repo(c: &mut Criterion) {
    // Note: Criterion's "Warming up for Xs" can look "stuck" if a single iteration takes longer
    // than the warm-up duration. Keep defaults moderate; scale up via env vars for stress runs.
    let commits = env_usize("GITGPUI_BENCH_COMMITS", 5_000);
    let local_branches = env_usize("GITGPUI_BENCH_LOCAL_BRANCHES", 200);
    let remote_branches = env_usize("GITGPUI_BENCH_REMOTE_BRANCHES", 800);
    let remotes = env_usize("GITGPUI_BENCH_REMOTES", 2);

    let fixture = OpenRepoFixture::new(commits, local_branches, remote_branches, remotes);

    let mut group = c.benchmark_group("open_repo");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.bench_with_input(
        BenchmarkId::new("long_history_and_branches", commits),
        &commits,
        |b, _| b.iter(|| fixture.run()),
    );
    group.finish();
}

fn bench_commit_details(c: &mut Criterion) {
    let files = env_usize("GITGPUI_BENCH_COMMIT_FILES", 5_000);
    let depth = env_usize("GITGPUI_BENCH_COMMIT_PATH_DEPTH", 4);
    let fixture = CommitDetailsFixture::new(files, depth);

    let mut group = c.benchmark_group("commit_details");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.bench_with_input(BenchmarkId::new("many_files", files), &files, |b, _| {
        b.iter(|| fixture.run())
    });
    group.finish();
}

fn bench_large_file_diff_scroll(c: &mut Criterion) {
    let lines = env_usize("GITGPUI_BENCH_DIFF_LINES", 10_000);
    let window = env_usize("GITGPUI_BENCH_DIFF_WINDOW", 200);
    let fixture = LargeFileDiffScrollFixture::new(lines);

    let mut group = c.benchmark_group("diff_scroll");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.bench_with_input(
        BenchmarkId::new("style_window", window),
        &window,
        |b, &window| {
            // Use a varying start index per-iteration to reduce cache effects in allocators.
            let mut start = 0usize;
            b.iter(|| {
                let h = fixture.run_scroll_step(start, window);
                start = start.wrapping_add(window) % lines.max(1);
                h
            })
        },
    );
    group.finish();
}

fn bench_conflict_three_way_scroll(c: &mut Criterion) {
    let lines = env_usize("GITGPUI_BENCH_CONFLICT_LINES", 10_000);
    let conflict_blocks = env_usize("GITGPUI_BENCH_CONFLICT_BLOCKS", 300);
    let window = env_usize("GITGPUI_BENCH_CONFLICT_WINDOW", 200);
    let fixture = ConflictThreeWayScrollFixture::new(lines, conflict_blocks);

    let mut group = c.benchmark_group("conflict_three_way_scroll");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.bench_with_input(
        BenchmarkId::new("style_window", window),
        &window,
        |b, &window| {
            let mut start = 0usize;
            b.iter(|| {
                let h = fixture.run_scroll_step(start, window);
                start = start.wrapping_add(window) % lines.max(1);
                h
            })
        },
    );
    group.finish();
}

fn bench_conflict_two_way_split_scroll(c: &mut Criterion) {
    let lines = env_usize("GITGPUI_BENCH_CONFLICT_LINES", 10_000);
    let conflict_blocks = env_usize("GITGPUI_BENCH_CONFLICT_BLOCKS", 300);
    let fixture = ConflictTwoWaySplitScrollFixture::new(lines, conflict_blocks);
    let windows = [100usize, 200, 400];

    let mut group = c.benchmark_group("conflict_two_way_split_scroll");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    for &window in &windows {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("window_{window}")),
            &window,
            |b, &window| {
                let mut start = 0usize;
                b.iter(|| {
                    let h = fixture.run_scroll_step(start, window);
                    start = start.wrapping_add(window) % fixture.visible_rows().max(1);
                    h
                })
            },
        );
    }
    group.finish();
}

fn bench_conflict_resolved_output_gutter_scroll(c: &mut Criterion) {
    let lines = env_usize("GITGPUI_BENCH_CONFLICT_LINES", 10_000);
    let conflict_blocks = env_usize("GITGPUI_BENCH_CONFLICT_BLOCKS", 300);
    let fixture = ConflictResolvedOutputGutterScrollFixture::new(lines, conflict_blocks);
    let windows = [100usize, 200, 400];

    let mut group = c.benchmark_group("conflict_resolved_output_gutter_scroll");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    for &window in &windows {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("window_{window}")),
            &window,
            |b, &window| {
                let mut start = 0usize;
                b.iter(|| {
                    let h = fixture.run_scroll_step(start, window);
                    start = start.wrapping_add(window) % fixture.visible_rows().max(1);
                    h
                })
            },
        );
    }
    group.finish();
}

fn bench_conflict_search_query_update(c: &mut Criterion) {
    let lines = env_usize("GITGPUI_BENCH_CONFLICT_LINES", 10_000);
    let conflict_blocks = env_usize("GITGPUI_BENCH_CONFLICT_BLOCKS", 300);
    let window = env_usize("GITGPUI_BENCH_CONFLICT_WINDOW", 200);
    let mut fixture = ConflictSearchQueryUpdateFixture::new(lines, conflict_blocks);
    let query_cycle = [
        "s", "sh", "sha", "shar", "share", "shared", "shared_", "shared_1",
    ];

    let mut group = c.benchmark_group("conflict_search_query_update");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.bench_with_input(BenchmarkId::new("window", window), &window, |b, &window| {
        let mut start = 0usize;
        let mut query_ix = 0usize;
        b.iter(|| {
            let query = query_cycle[query_ix % query_cycle.len()];
            let h = fixture.run_query_update_step(query, start, window);
            query_ix = query_ix.wrapping_add(1);
            start = start.wrapping_add(window.max(1) / 2 + 1) % fixture.visible_rows().max(1);
            h
        })
    });
    group.finish();
}

fn bench_conflict_split_resize_step(c: &mut Criterion) {
    let lines = env_usize("GITGPUI_BENCH_CONFLICT_LINES", 10_000);
    let conflict_blocks = env_usize("GITGPUI_BENCH_CONFLICT_BLOCKS", 300);
    let window = env_usize("GITGPUI_BENCH_CONFLICT_WINDOW", 200);
    let resize_query =
        env::var("GITGPUI_BENCH_CONFLICT_RESIZE_QUERY").unwrap_or_else(|_| "shared".to_string());
    let mut fixture = ConflictSplitResizeStepFixture::new(lines, conflict_blocks);

    let mut group = c.benchmark_group("conflict_split_resize_step");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.bench_with_input(BenchmarkId::new("window", window), &window, |b, &window| {
        let mut start = 0usize;
        b.iter(|| {
            let h = fixture.run_resize_step(resize_query.as_str(), start, window);
            start = start.wrapping_add(window.max(1) / 3 + 1) % fixture.visible_rows().max(1);
            h
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_open_repo,
    bench_commit_details,
    bench_large_file_diff_scroll,
    bench_conflict_three_way_scroll,
    bench_conflict_two_way_split_scroll,
    bench_conflict_resolved_output_gutter_scroll,
    bench_conflict_search_query_update,
    bench_conflict_split_resize_step
);
criterion_main!(benches);
