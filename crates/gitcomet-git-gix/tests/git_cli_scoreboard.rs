use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, ExprCall, ExprLit, File, ImplItemFn, ItemFn, ItemImpl, ItemMod, Lit};

const GIT_COMMAND_BUDGET: usize = 105;

#[test]
fn production_git_cli_call_scoreboard_with_budget_gate() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_root = crate_root.join("src");
    let files = rust_source_files(&src_root);

    assert!(
        !files.is_empty(),
        "expected Rust source files under {}",
        src_root.display()
    );

    let mut per_file = Vec::new();
    let mut total = 0usize;

    for file in files {
        let source = fs::read_to_string(&file)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", file.display()));
        let parsed: File = syn::parse_file(&source)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", file.display()));

        let mut counter = GitCommandCounter::default();
        counter.visit_file(&parsed);

        if counter.count > 0 {
            let rel = file
                .strip_prefix(&crate_root)
                .unwrap_or(file.as_path())
                .to_path_buf();
            per_file.push((rel, counter.count));
            total += counter.count;
        }
    }

    per_file.sort_by(|(a_path, a_count), (b_path, b_count)| {
        b_count.cmp(a_count).then_with(|| a_path.cmp(b_path))
    });

    eprintln!("git CLI migration scoreboard: production Command::new(\"git\") calls = {total}");
    for (path, count) in &per_file {
        eprintln!("  {count:>3} {}", path.display());
    }
    eprintln!("budget gate: <= {GIT_COMMAND_BUDGET}");

    assert!(
        total <= GIT_COMMAND_BUDGET,
        "production Command::new(\"git\") call count increased to {total} (budget {GIT_COMMAND_BUDGET}). \
         Reduce CLI usage or explicitly lower the budget after migration progress."
    );
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_source_files(root, &mut files);
    files.sort();
    files
}

fn collect_rust_source_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read source directory {}: {err}", dir.display()))
        .map(|entry| entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}")))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

#[derive(Default)]
struct GitCommandCounter {
    count: usize,
}

impl<'ast> Visit<'ast> for GitCommandCounter {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit::visit_item_mod(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit::visit_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit::visit_item_impl(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        if has_test_cfg(&node.attrs) {
            return;
        }
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if is_git_command_new_call(node) {
            self.count += 1;
        }
        visit::visit_expr_call(self, node);
    }
}

fn has_test_cfg(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        match &attr.meta {
            syn::Meta::List(list) => list
                .tokens
                .to_string()
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .any(|token| token == "test"),
            _ => false,
        }
    })
}

fn is_git_command_new_call(expr: &ExprCall) -> bool {
    let Expr::Path(path_expr) = expr.func.as_ref() else {
        return false;
    };

    let mut segments = path_expr.path.segments.iter().rev();
    let Some(last) = segments.next() else {
        return false;
    };
    let Some(previous) = segments.next() else {
        return false;
    };
    if last.ident != "new" || previous.ident != "Command" {
        return false;
    }

    let Some(first_arg) = expr.args.first() else {
        return false;
    };
    let Expr::Lit(ExprLit {
        lit: Lit::Str(lit_str),
        ..
    }) = first_arg
    else {
        return false;
    };

    lit_str.value() == "git"
}
