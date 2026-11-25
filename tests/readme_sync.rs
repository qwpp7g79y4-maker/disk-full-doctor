//! Keeps `README.md` and `README.nl.md` in step: same fenced code blocks and
//! same inline code spans (commands, flags, exit codes, check names).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn read(name: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(name)).unwrap()
}

fn fenced_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            match current.take() {
                Some(lines) => blocks.push(lines.join("\n")),
                None => current = Some(Vec::new()),
            }
        } else if let Some(lines) = current.as_mut() {
            lines.push(line);
        }
    }
    assert!(current.is_none(), "unterminated code fence");
    blocks
}

fn inline_spans(text: &str) -> BTreeSet<String> {
    let mut spans = BTreeSet::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        } else if !in_fence {
            for (i, part) in line.split('`').enumerate() {
                if i % 2 == 1 {
                    spans.insert(part.to_string());
                }
            }
        }
    }
    spans
}

#[test]
fn readmes_have_the_same_code_blocks() {
    let en = fenced_blocks(&read("README.md"));
    assert!(!en.is_empty());
    assert_eq!(en, fenced_blocks(&read("README.nl.md")));
}

#[test]
fn readmes_have_the_same_inline_code() {
    let en = inline_spans(&read("README.md"));
    let nl = inline_spans(&read("README.nl.md"));
    let only_en: Vec<_> = en.difference(&nl).collect();
    let only_nl: Vec<_> = nl.difference(&en).collect();
    assert!(
        only_en.is_empty() && only_nl.is_empty(),
        "only in README.md: {only_en:?}; only in README.nl.md: {only_nl:?}"
    );
}

#[test]
fn readmes_link_to_each_other() {
    assert!(read("README.md").contains("(README.nl.md)"));
    assert!(read("README.nl.md").contains("(README.md)"));
}
