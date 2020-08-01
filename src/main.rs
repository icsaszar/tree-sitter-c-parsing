use ignore::DirEntry;
use rayon::prelude::*;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::fs::FileType;
use std::path::PathBuf;
use std::sync::Arc;
use tree_sitter::{Parser, Query, QueryCapture, QueryCursor, Range};

extern "C" {
    fn tree_sitter_c() -> tree_sitter::Language;
}

pub fn get_parser() -> Parser {
    let parser_language = unsafe { tree_sitter_c() };
    let mut parser = Parser::new();
    parser.set_language(parser_language).unwrap();
    parser
}

pub fn collect_files_and_extensions(
    root_dir: &PathBuf,
) -> Vec<PathBuf> {
    let mut types = ignore::types::TypesBuilder::new();
    types.add("c", "*.c").unwrap();
    types.add("h", "*.h").unwrap();
    types.select("all");
    let types = types.build().unwrap();

    let mut walker = ignore::WalkBuilder::new(&root_dir);
    walker.types(types);

    let walker = walker.build();

    walker
        .filter_map(Result::ok)
        .filter(|e| DirEntry::file_type(e).map_or(false, |ft| FileType::is_file(&ft)))
        .map(DirEntry::into_path)
        .collect()
}

fn parse_folder(folder: &PathBuf) -> (usize, Vec<(PathBuf, Vec<Range>)>) {
    let src = folder.join("src");
    let folder = if src.exists() { &src } else { folder };

    let files = collect_files_and_extensions(folder);

    let ts_language = unsafe { tree_sitter_c() };
    let query = Arc::new(Query::new(ts_language, "(ERROR) @error").unwrap());
    let total = files.len();
    let errs: Vec<(PathBuf, Vec<Range>)> = files
        .into_par_iter()
        .filter_map(|path| {
            std::thread_local! {
                static PARSER: RefCell<Parser> = {
                    let parser = get_parser();
                    RefCell::new(parser)
                };
                static QUERY_CURSOR: RefCell<QueryCursor> = RefCell::new(QueryCursor::new());
            };
            PARSER.with(|parser| {
                let mut parser = parser.borrow_mut();
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    let tree = parser.parse(&contents, None).unwrap();
                    let root_node = tree.root_node();
                    QUERY_CURSOR.with(|cursor| {
                        if tree.root_node().has_error() {
                            let mut cursor = cursor.borrow_mut();
                            let query = query.borrow();
                            let matches = cursor.matches(query, root_node, |node| {
                                node.utf8_text(contents.as_bytes()).unwrap()
                            });

                            let errs: Vec<tree_sitter::Range> = matches
                                .flat_map(|m| m.captures.iter())
                                .map(|&QueryCapture { node, .. }| node.range())
                                .collect();

                            Some((path, errs))
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
        })
        .collect();

    (total, errs)
}

fn parse_repos(root_dir: PathBuf) {
    let errs: Vec<_> = std::fs::read_dir(&root_dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            entry.path()
        })
        .filter(|path| path.is_dir())
        .map(|path| (path.clone(), parse_folder(&path)))
        .collect();
    for (proj, (total, errs)) in errs {
        println!("{:?}", proj);
        let nr_errs = errs.len();
        println!("\t {} / {} (errs/total)", nr_errs, total);
        if errs.len() < 10 {
            for (file, errs) in errs {
                println!("\t{:?}", file);
                println!("\t\t{}", errs.len());
            }
        }
    }
}

fn main() {
    let repos = "./repos".into();
    parse_repos(repos)
}
