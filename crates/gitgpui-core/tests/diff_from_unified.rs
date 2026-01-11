use gitgpui_core::domain::{Diff, DiffArea, DiffLineKind, DiffTarget};
use std::path::PathBuf;

#[test]
fn diff_from_unified_classifies_lines() {
    let target = DiffTarget::WorkingTree {
        path: PathBuf::from("a.txt"),
        area: DiffArea::Unstaged,
    };

    let text = "\
diff --git a/a.txt b/a.txt
index 0000000..1111111 100644
--- a/a.txt
+++ b/a.txt
@@ -0,0 +1,2 @@
+hello
 world
-bye
";

    let diff = Diff::from_unified(target, text);
    assert!(diff.lines.iter().any(|l| l.kind == DiffLineKind::Header));
    assert!(diff.lines.iter().any(|l| l.kind == DiffLineKind::Hunk));
    assert!(diff.lines.iter().any(|l| l.kind == DiffLineKind::Add));
    assert!(diff.lines.iter().any(|l| l.kind == DiffLineKind::Remove));
    assert!(diff.lines.iter().any(|l| l.kind == DiffLineKind::Context));
}
