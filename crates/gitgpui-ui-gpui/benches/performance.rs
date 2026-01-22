use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gitgpui_ui_gpui::benchmarks::{CommitDetailsFixture, LargeFileDiffScrollFixture, OpenRepoFixture};
use std::env;

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn bench_open_repo(c: &mut Criterion) {
    let commits = env_usize("GITGPUI_BENCH_COMMITS", 100_000);
    let local_branches = env_usize("GITGPUI_BENCH_LOCAL_BRANCHES", 5_000);
    let remote_branches = env_usize("GITGPUI_BENCH_REMOTE_BRANCHES", 20_000);
    let remotes = env_usize("GITGPUI_BENCH_REMOTES", 3);

    let fixture = OpenRepoFixture::new(commits, local_branches, remote_branches, remotes);

    let mut group = c.benchmark_group("open_repo");
    group.bench_with_input(
        BenchmarkId::new("long_history_and_branches", commits),
        &commits,
        |b, _| b.iter(|| fixture.run()),
    );
    group.finish();
}

fn bench_commit_details(c: &mut Criterion) {
    let files = env_usize("GITGPUI_BENCH_COMMIT_FILES", 50_000);
    let depth = env_usize("GITGPUI_BENCH_COMMIT_PATH_DEPTH", 4);
    let fixture = CommitDetailsFixture::new(files, depth);

    let mut group = c.benchmark_group("commit_details");
    group.bench_with_input(
        BenchmarkId::new("many_files", files),
        &files,
        |b, _| b.iter(|| fixture.run()),
    );
    group.finish();
}

fn bench_large_file_diff_scroll(c: &mut Criterion) {
    let lines = env_usize("GITGPUI_BENCH_DIFF_LINES", 100_000);
    let window = env_usize("GITGPUI_BENCH_DIFF_WINDOW", 200);
    let fixture = LargeFileDiffScrollFixture::new(lines);

    let mut group = c.benchmark_group("diff_scroll");
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

criterion_group!(
    benches,
    bench_open_repo,
    bench_commit_details,
    bench_large_file_diff_scroll
);
criterion_main!(benches);
