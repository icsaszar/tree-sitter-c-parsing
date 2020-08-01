use std::path::PathBuf;

fn main() {
    let dir: PathBuf = ["tree-sitter-c", "src"].iter().collect();
    let file = dir.join("parser.c");
    cc::Build::new()
        .include(&dir)
        .file(file)
        .compile("tree-sitter-c");
}
