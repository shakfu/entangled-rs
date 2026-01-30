//! Performance benchmarks for Entangled

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use entangled::config::{Config, NamespaceDefault};
use entangled::model::{tangle_ref, CodeBlock, ReferenceId, ReferenceMap, ReferenceName};
use entangled::readers::parse_markdown;
use entangled::text_location::TextLocation;

fn generate_markdown(num_blocks: usize, lines_per_block: usize) -> String {
    let mut md = String::from("# Benchmark Document\n\n");

    // Create main block that references others
    md.push_str("```python #main file=output.py\n");
    for i in 0..num_blocks {
        md.push_str(&format!("<<block{}>>\n", i));
    }
    md.push_str("```\n\n");

    // Create referenced blocks
    for i in 0..num_blocks {
        md.push_str(&format!("```python #block{}\n", i));
        for j in 0..lines_per_block {
            md.push_str(&format!("print('Block {} line {}')\n", i, j));
        }
        md.push_str("```\n\n");
    }

    md
}

fn generate_nested_markdown(depth: usize, breadth: usize) -> String {
    let mut md = String::from("# Nested Benchmark\n\n");

    fn generate_block(md: &mut String, prefix: &str, depth: usize, breadth: usize, is_root: bool) {
        let name = if prefix.is_empty() { "main".to_string() } else { prefix.to_string() };
        let file_attr = if is_root { " file=output.py" } else { "" };

        md.push_str(&format!("```python #{}{}\n", name, file_attr));

        if depth > 0 {
            for i in 0..breadth {
                let child_name = if prefix.is_empty() {
                    format!("child{}", i)
                } else {
                    format!("{}_{}", prefix, i)
                };
                md.push_str(&format!("<<{}>>\n", child_name));
            }
        } else {
            md.push_str("pass\n");
        }

        md.push_str("```\n\n");

        if depth > 0 {
            for i in 0..breadth {
                let child_name = if prefix.is_empty() {
                    format!("child{}", i)
                } else {
                    format!("{}_{}", prefix, i)
                };
                generate_block(md, &child_name, depth - 1, breadth, false);
            }
        }
    }

    generate_block(&mut md, "", depth, breadth, true);
    md
}

fn bench_parse_markdown(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_markdown");

    let mut config = Config::default();
    config.namespace_default = NamespaceDefault::None;

    for num_blocks in [10, 50, 100, 500].iter() {
        let md = generate_markdown(*num_blocks, 10);
        group.bench_with_input(
            BenchmarkId::new("blocks", num_blocks),
            &md,
            |b, md| {
                b.iter(|| {
                    parse_markdown(black_box(md), None, &config).unwrap()
                })
            },
        );
    }

    group.finish();
}

fn bench_tangle(c: &mut Criterion) {
    let mut group = c.benchmark_group("tangle");

    let mut config = Config::default();
    config.namespace_default = NamespaceDefault::None;

    for num_blocks in [10, 50, 100, 500].iter() {
        let md = generate_markdown(*num_blocks, 10);
        let doc = parse_markdown(&md, None, &config).unwrap();

        group.bench_with_input(
            BenchmarkId::new("blocks", num_blocks),
            &doc.refs,
            |b, refs| {
                b.iter(|| {
                    tangle_ref(black_box(refs), &ReferenceName::new("main"), None, None).unwrap()
                })
            },
        );
    }

    group.finish();
}

fn bench_tangle_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("tangle_nested");

    let mut config = Config::default();
    config.namespace_default = NamespaceDefault::None;

    // Test different nesting depths with breadth=3
    for depth in [2, 3, 4, 5].iter() {
        let md = generate_nested_markdown(*depth, 3);
        let doc = parse_markdown(&md, None, &config).unwrap();
        let total_blocks = doc.refs.len();

        group.bench_with_input(
            BenchmarkId::new("depth", format!("d{}({}blks)", depth, total_blocks)),
            &doc.refs,
            |b, refs| {
                b.iter(|| {
                    tangle_ref(black_box(refs), &ReferenceName::new("main"), None, None).unwrap()
                })
            },
        );
    }

    group.finish();
}

fn bench_reference_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("reference_map");

    for num_blocks in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("insert", num_blocks),
            num_blocks,
            |b, &n| {
                b.iter(|| {
                    let mut refs = ReferenceMap::new();
                    for i in 0..n {
                        let block = CodeBlock::new(
                            ReferenceId::first(ReferenceName::new(format!("block{}", i))),
                            Some("python".to_string()),
                            format!("print({})", i),
                            TextLocation::default(),
                        );
                        refs.insert(black_box(block));
                    }
                    refs
                })
            },
        );
    }

    // Lookup benchmark
    let mut refs = ReferenceMap::new();
    for i in 0..10000 {
        let block = CodeBlock::new(
            ReferenceId::first(ReferenceName::new(format!("block{}", i))),
            Some("python".to_string()),
            format!("print({})", i),
            TextLocation::default(),
        );
        refs.insert(block);
    }

    group.bench_function("lookup_10k", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let name = ReferenceName::new(format!("block{}", i * 10));
                black_box(refs.get_by_name(&name));
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_markdown,
    bench_tangle,
    bench_tangle_nested,
    bench_reference_map,
);

criterion_main!(benches);
