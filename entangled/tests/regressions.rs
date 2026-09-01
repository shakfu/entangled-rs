//! End-to-end regression tests for the defects found in the project review.
//!
//! Each test drives a real temporary project through parse -> analyze ->
//! tangle/stitch -> persistence, which is where these defects lived: none of
//! them were visible from a single module's unit tests.

use std::fs;
use std::path::Path;

use entangled::config::{AnnotationMethod, Config, NamespaceDefault};
use entangled::interface::{
    analyze_project, stitch_documents, sync_documents, tangle_documents, Context,
};

/// Creates a project directory containing the given `(path, contents)` files.
fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (name, contents) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }
    dir
}

fn context(dir: &Path, config: Config) -> Context {
    Context::new(config, dir.to_path_buf()).unwrap()
}

fn global_ns() -> Config {
    Config {
        namespace_default: NamespaceDefault::None,
        ..Config::default()
    }
}

// --- F1: target collisions must not discard code -----------------------------

#[test]
fn tangle_rejects_two_block_names_writing_one_target() {
    let dir = project(&[(
        "doc.md",
        "```python #a file=out.py\nprint('A')\n```\n\n```python #b file=out.py\nprint('B')\n```\n",
    )]);
    let ctx = context(dir.path(), global_ns());

    let err = tangle_documents(&ctx).expect_err("collision must be refused");
    let message = err.to_string();
    assert!(message.contains("out.py"), "unhelpful message: {message}");
    assert!(
        message.contains("`a`") && message.contains("`b`"),
        "{message}"
    );
}

#[test]
fn tangle_leaves_every_file_untouched_when_a_target_collides() {
    // The pre-existing file must survive: a partial tangle that had already
    // written some other target would be worse than refusing outright.
    let dir = project(&[
        (
            "doc.md",
            "```python #a file=out.py\nprint('A')\n```\n\n\
             ```python #b file=out.py\nprint('B')\n```\n\n\
             ```python #ok file=fine.py\nprint('ok')\n```\n",
        ),
        ("fine.py", "PRE-EXISTING\n"),
    ]);
    let ctx = context(dir.path(), global_ns());

    assert!(tangle_documents(&ctx).is_err());
    assert!(!dir.path().join("out.py").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("fine.py")).unwrap(),
        "PRE-EXISTING\n"
    );
}

#[test]
fn continuation_blocks_may_share_one_target() {
    // Same *name* twice is the supported multi-block case and must still work.
    let dir = project(&[(
        "doc.md",
        "```python #main file=out.py\nfirst()\n```\n\n```python #main\nsecond()\n```\n",
    )]);
    let mut ctx = context(dir.path(), global_ns());

    let tx = tangle_documents(&ctx).unwrap();
    tx.execute(&mut ctx.filedb).unwrap();

    let out = fs::read_to_string(dir.path().join("out.py")).unwrap();
    assert!(out.contains("first()") && out.contains("second()"), "{out}");
}

// --- F2: block identity is project-wide --------------------------------------

#[test]
fn same_name_blocks_in_different_files_stay_distinct() {
    let dir = project(&[
        (
            "a.md",
            "```python #part\nprint('from A')\n```\n\n\
             ```python #main file=out.py\n<<part>>\n```\n",
        ),
        ("b.md", "```python #part\nprint('from B')\n```\n"),
    ]);
    let mut ctx = context(dir.path(), global_ns());

    let tx = tangle_documents(&ctx).unwrap();
    tx.execute(&mut ctx.filedb).unwrap();

    // With per-document IDs both blocks were `part[0]`, so one replaced the
    // other and the survivor was expanded twice.
    let out = fs::read_to_string(dir.path().join("out.py")).unwrap();
    assert_eq!(out.matches("from A").count(), 1, "{out}");
    assert_eq!(out.matches("from B").count(), 1, "{out}");
}

#[test]
fn every_block_id_in_the_project_is_unique() {
    let dir = project(&[
        ("a.md", "```python #part\na()\n```\n"),
        ("b.md", "```python #part\nb()\n```\n"),
        ("c.md", "```python #part\nc()\n```\n"),
    ]);
    let ctx = context(dir.path(), global_ns());

    let analysis = analyze_project(&ctx).unwrap();
    assert_eq!(analysis.refs.len(), 3);
    assert_eq!(analysis.locations.len(), 3);
    let sources: Vec<&str> = analysis.refs.blocks().map(|b| b.source.as_str()).collect();
    assert_eq!(sources, vec!["a()", "b()", "c()"]);
}

#[test]
fn duplicate_basenames_in_different_directories_get_distinct_namespaces() {
    // The default file namespace used to key on the file name alone, so
    // `chapter/a.md` and `other/a.md` shared one namespace.
    let dir = project(&[
        (
            "chapter/a.md",
            "```python #part\nprint('chapter')\n```\n\n\
             ```python #main file=out.py\n<<part>>\n<<other/a.md#part>>\n```\n",
        ),
        ("other/a.md", "```python #part\nprint('other')\n```\n"),
    ]);
    let mut ctx = context(dir.path(), Config::default());

    let tx = tangle_documents(&ctx).unwrap();
    tx.execute(&mut ctx.filedb).unwrap();

    let out = fs::read_to_string(dir.path().join("out.py")).unwrap();
    assert!(out.contains("print('chapter')"), "{out}");
    assert!(out.contains("print('other')"), "{out}");
}

#[test]
fn a_bare_reference_resolves_inside_its_own_file_namespace() {
    // Under the default namespace a block in `a.md` is named `a.md#part`, so
    // `<<part>>` has to resolve within the document before falling back.
    let dir = project(&[(
        "a.md",
        "```python #part\nprint('p')\n```\n\n```python #main file=out.py\n<<part>>\n```\n",
    )]);
    let mut ctx = context(dir.path(), Config::default());

    let tx = tangle_documents(&ctx).unwrap();
    tx.execute(&mut ctx.filedb).unwrap();

    assert!(fs::read_to_string(dir.path().join("out.py"))
        .unwrap()
        .contains("print('p')"));
}

// --- F3: file headers are emitted once ---------------------------------------

fn header_hooks() -> Config {
    let mut config = global_ns();
    config.hooks.shebang = true;
    config.hooks.spdx_license = true;
    config
}

#[test]
fn shebang_and_spdx_headers_appear_exactly_once_at_the_top() {
    let dir = project(&[(
        "doc.md",
        "```python #main file=out.py\n\
         #!/usr/bin/env python3\n\
         # SPDX-License-Identifier: MIT\n\
         print(1)\n```\n",
    )]);
    let mut ctx = context(dir.path(), header_hooks());

    let tx = tangle_documents(&ctx).unwrap();
    tx.execute(&mut ctx.filedb).unwrap();

    let out = fs::read_to_string(dir.path().join("out.py")).unwrap();
    assert_eq!(out.matches("#!/usr/bin/env python3").count(), 1, "{out}");
    assert_eq!(out.matches("SPDX-License-Identifier").count(), 1, "{out}");
    assert!(
        out.starts_with("#!/usr/bin/env python3\n# SPDX-License-Identifier: MIT\n"),
        "{out}"
    );
    // Both must be hoisted above the annotations, not left inside the block.
    let first_marker = out.find("~/~ begin").unwrap();
    assert!(
        out.find("SPDX-License-Identifier").unwrap() < first_marker,
        "{out}"
    );
}

#[test]
fn a_shebang_above_an_spdx_line_does_not_hide_it() {
    // The SPDX hook only looks at the top of the source, so it used to miss a
    // licence line sitting under a shebang. Chaining the pre-hooks fixes that.
    let dir = project(&[(
        "doc.md",
        "```python #main file=out.py\n#!/bin/sh\n# SPDX-License-Identifier: MIT\ntrue\n```\n",
    )]);
    let mut ctx = context(dir.path(), header_hooks());

    let tx = tangle_documents(&ctx).unwrap();
    tx.execute(&mut ctx.filedb).unwrap();

    let out = fs::read_to_string(dir.path().join("out.py")).unwrap();
    assert!(
        out.starts_with("#!/bin/sh\n# SPDX-License-Identifier: MIT\n"),
        "{out}"
    );
}

#[test]
fn continuation_blocks_do_not_each_contribute_a_header() {
    let dir = project(&[(
        "doc.md",
        "```python #main file=out.py\n#!/bin/sh\nfirst\n```\n\n```python #main\nsecond\n```\n",
    )]);
    let mut ctx = context(dir.path(), header_hooks());

    let tx = tangle_documents(&ctx).unwrap();
    tx.execute(&mut ctx.filedb).unwrap();

    let out = fs::read_to_string(dir.path().join("out.py")).unwrap();
    assert_eq!(out.matches("#!/bin/sh").count(), 1, "{out}");
    assert!(out.contains("first") && out.contains("second"), "{out}");
}

#[test]
fn hooks_survive_a_tangle_stitch_round_trip() {
    let dir = project(&[(
        "doc.md",
        "```python #main file=out.py\n#!/bin/sh\necho one\n```\n",
    )]);
    let mut ctx = context(dir.path(), header_hooks());
    sync_documents(&mut ctx, false).unwrap();

    // Edit the generated file, then stitch the edit back.
    let out_path = dir.path().join("out.py");
    let edited = fs::read_to_string(&out_path)
        .unwrap()
        .replace("echo one", "echo two");
    fs::write(&out_path, edited).unwrap();

    let tx = stitch_documents(&ctx).unwrap();
    tx.execute_force(&mut ctx.filedb).unwrap();

    // The shebang lives outside the annotated block now, so stitch must not
    // drag it back into the markdown, and it must not be lost either.
    let markdown = fs::read_to_string(dir.path().join("doc.md")).unwrap();
    assert!(markdown.contains("echo two"), "{markdown}");
    assert_eq!(markdown.matches("#!/bin/sh").count(), 1, "{markdown}");

    sync_documents(&mut ctx, true).unwrap();
    let out = fs::read_to_string(&out_path).unwrap();
    assert_eq!(out.matches("#!/bin/sh").count(), 1, "{out}");
}

// --- F6/F8: one target resolver, with a containment policy -------------------

#[test]
fn output_dir_prefixes_generated_targets() {
    let dir = project(&[("doc.md", "```python #main file=main.py\nprint(1)\n```\n")]);
    let mut config = global_ns();
    config.output_dir = Some("generated".into());
    let mut ctx = context(dir.path(), config);

    tangle_documents(&ctx)
        .unwrap()
        .execute(&mut ctx.filedb)
        .unwrap();

    assert!(dir.path().join("generated/main.py").exists());
    assert!(!dir.path().join("main.py").exists());
}

#[test]
fn output_dir_round_trips_through_stitch() {
    // Stitch has to look for the generated file in the same place tangle put
    // it, or an `output_dir` project silently stops syncing back.
    let dir = project(&[("doc.md", "```python #main file=main.py\nprint(1)\n```\n")]);
    let mut config = global_ns();
    config.output_dir = Some("generated".into());
    let mut ctx = context(dir.path(), config);
    sync_documents(&mut ctx, false).unwrap();

    let out_path = dir.path().join("generated/main.py");
    let edited = fs::read_to_string(&out_path)
        .unwrap()
        .replace("print(1)", "print(2)");
    fs::write(&out_path, edited).unwrap();

    let tx = stitch_documents(&ctx).unwrap();
    assert!(!tx.is_empty(), "stitch did not see the edit");
    tx.execute_force(&mut ctx.filedb).unwrap();
    assert!(fs::read_to_string(dir.path().join("doc.md"))
        .unwrap()
        .contains("print(2)"));
}

#[test]
fn traversal_targets_are_rejected_by_default() {
    let dir = project(&[(
        "doc.md",
        "```python #evil file=../escaped.py\nprint('escaped')\n```\n",
    )]);
    let ctx = context(dir.path(), global_ns());

    let err = tangle_documents(&ctx).expect_err("traversal must be refused");
    assert!(err.to_string().contains("outside the project directory"));
    assert!(!dir.path().parent().unwrap().join("escaped.py").exists());
}

#[test]
fn absolute_targets_are_rejected_by_default() {
    let outside = tempfile::tempdir().unwrap();
    let victim = outside.path().join("victim.py");
    let dir = project(&[(
        "doc.md",
        &format!(
            "```python #evil file={}\nprint('escaped')\n```\n",
            victim.display()
        ),
    )]);
    let ctx = context(dir.path(), global_ns());

    assert!(tangle_documents(&ctx).is_err());
    assert!(!victim.exists());
}

#[test]
fn external_targets_are_allowed_when_explicitly_opted_in() {
    let dir = project(&[(
        "doc.md",
        "```python #out file=../escaped.py\nprint('deliberate')\n```\n",
    )]);
    let mut config = global_ns();
    config.allow_external_targets = true;
    let mut ctx = context(dir.path(), config);

    tangle_documents(&ctx)
        .unwrap()
        .execute(&mut ctx.filedb)
        .unwrap();
    let escaped = dir.path().parent().unwrap().join("escaped.py");
    assert!(escaped.exists());
    fs::remove_file(escaped).unwrap();
}

// --- F9: line endings are preserved ------------------------------------------

#[test]
fn stitch_preserves_crlf_line_endings() {
    let dir = project(&[(
        "doc.md",
        "---\r\ntitle: T\r\n---\r\n\r\n```python #main file=out.py\r\nprint(1)\r\n```\r\n",
    )]);
    let mut config = global_ns();
    config.annotation = AnnotationMethod::Standard;
    let mut ctx = context(dir.path(), config);
    sync_documents(&mut ctx, false).unwrap();

    let out_path = dir.path().join("out.py");
    let edited = fs::read_to_string(&out_path)
        .unwrap()
        .replace("print(1)", "print(2)");
    fs::write(&out_path, edited).unwrap();

    let tx = stitch_documents(&ctx).unwrap();
    tx.execute_force(&mut ctx.filedb).unwrap();

    let markdown = fs::read_to_string(dir.path().join("doc.md")).unwrap();
    assert!(markdown.contains("print(2)"), "{markdown:?}");
    // The whole file must still be CRLF: every `\n` is preceded by a `\r`.
    assert_eq!(
        markdown.matches('\n').count(),
        markdown.matches("\r\n").count(),
        "line endings were normalised to LF: {markdown:?}"
    );
    // The fence and frontmatter must survive: a wrong YAML offset used to
    // splice over the closing fence instead of the block body.
    assert!(
        markdown.starts_with("---\r\ntitle: T\r\n---\r\n"),
        "{markdown:?}"
    );
    assert!(markdown.trim_end().ends_with("```"), "{markdown:?}");
    assert!(!markdown.contains("print(1)"), "{markdown:?}");
}

// --- F7: a transaction is all-or-nothing -------------------------------------

mod transaction_atomicity {
    use std::fs;
    use std::path::{Path, PathBuf};

    use entangled::errors::{EntangledError, Result};
    use entangled::io::{Action, FileDB, Transaction};

    /// An action that always fails at commit time, to exercise rollback.
    #[derive(Debug)]
    struct AlwaysFails {
        path: PathBuf,
    }

    impl Action for AlwaysFails {
        fn target(&self) -> &Path {
            &self.path
        }
        fn check_conflict(&self, _db: &FileDB) -> Result<()> {
            Ok(())
        }
        fn execute(&self) -> Result<()> {
            Err(EntangledError::Other("injected failure".to_string()))
        }
        fn update_db(&self, _db: &mut FileDB) -> Result<()> {
            Ok(())
        }
        fn describe(&self) -> String {
            "always fails".to_string()
        }
    }

    #[test]
    fn a_failing_action_rolls_back_the_earlier_ones() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("existing.txt");
        let created = dir.path().join("created.txt");
        fs::write(&existing, "ORIGINAL\n").unwrap();

        let mut tx = Transaction::new();
        tx.write(&existing, "REPLACED\n");
        tx.write(&created, "NEW\n");
        tx.add(AlwaysFails {
            path: dir.path().join("boom.txt"),
        });

        let mut db = FileDB::new();
        let err = tx.execute(&mut db).expect_err("the third action must fail");
        assert!(err.to_string().contains("injected failure"), "{err}");

        // A file that existed is back to its original contents...
        assert_eq!(fs::read_to_string(&existing).unwrap(), "ORIGINAL\n");
        // ...and a file the transaction created is gone again.
        assert!(!created.exists());
        // The database must not claim files that were rolled back.
        assert!(db.is_empty());
    }

    #[test]
    fn a_successful_transaction_leaves_no_scratch_files() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out.txt");
        fs::write(&target, "OLD\n").unwrap();

        let mut tx = Transaction::new();
        tx.write(&target, "NEW\n");

        let mut db = FileDB::new();
        tx.execute(&mut db).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "NEW\n");
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".entangled-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "scratch files left behind: {leftovers:?}"
        );
    }
}

// --- F5: the evaluation cache is keyed on the resolved command ---------------

#[test]
fn changing_a_runner_argument_invalidates_the_cache() {
    use entangled::eval::{eval_documents, EvalOptions};

    let dir = project(&[("doc.md", "```sh #demo eval=demo\necho ignored\n```\n")]);

    let mut config = global_ns();
    config.eval.runners.insert(
        "demo".into(),
        vec!["sh".into(), "-c".into(), "echo one".into()],
    );
    let ctx = context(dir.path(), config);
    let first = eval_documents(&ctx, &EvalOptions::default()).unwrap();
    assert_eq!(first[0].stdout.trim(), "one");

    // Same block text, different runner command. Hashing only the runner's
    // *name* left the hash unchanged, so the stale "one" was served.
    let mut config = global_ns();
    config.eval.runners.insert(
        "demo".into(),
        vec!["sh".into(), "-c".into(), "echo two".into()],
    );
    let ctx = context(dir.path(), config);
    let second = eval_documents(&ctx, &EvalOptions::default()).unwrap();
    assert_eq!(second[0].stdout.trim(), "two");
}

#[test]
fn an_evaluated_block_runs_in_the_project_directory() {
    use entangled::eval::{eval_documents, EvalOptions};

    // Read a project file by relative path rather than comparing `pwd` output:
    // under Git bash on Windows `pwd` prints an MSYS path (/c/Users/...) that
    // no Windows API resolves.
    let dir = project(&[
        ("marker.txt", "in-project"),
        ("doc.md", "```sh #where eval=sh\ncat marker.txt\n```\n"),
    ]);
    let ctx = context(dir.path(), global_ns());

    let results = eval_documents(&ctx, &EvalOptions::default()).unwrap();
    assert_eq!(results[0].stdout.trim(), "in-project", "{:?}", results[0]);
}

#[test]
fn a_block_that_never_exits_is_killed_at_the_timeout() {
    use entangled::eval::{eval_documents, EvalOptions};

    let dir = project(&[("doc.md", "```sh #forever eval=sh\nsleep 30\n```\n")]);
    let mut config = global_ns();
    config.eval.timeout_secs = 1;
    let ctx = context(dir.path(), config);

    let started = std::time::Instant::now();
    let results = eval_documents(&ctx, &EvalOptions::default()).unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
    assert_eq!(results[0].exit_code, None);
    assert!(results[0].stderr.contains("timeout"), "{:?}", results[0]);
}

#[test]
fn a_block_producing_more_output_than_a_pipe_holds_does_not_deadlock() {
    use entangled::eval::{eval_documents, EvalOptions};

    // Well past the 64 KiB pipe buffer. Writing all of stdin before reading any
    // output used to block here forever.
    let dir = project(&[(
        "doc.md",
        "```sh #loud eval=sh\nhead -c 500000 /dev/zero | tr '\\0' 'x'\n```\n",
    )]);
    let mut config = global_ns();
    config.eval.timeout_secs = 30;
    let ctx = context(dir.path(), config);

    let results = eval_documents(&ctx, &EvalOptions::default()).unwrap();
    assert_eq!(results[0].exit_code, Some(0), "{:?}", results[0]);
    assert_eq!(results[0].stdout.len(), 500_000);
}

// --- F10: HTML anchors are unique --------------------------------------------

#[test]
fn names_that_normalise_identically_get_distinct_anchors() {
    use entangled::{weave_document, HtmlOptions};

    // `a.b` and `a-b` both reduce to the slug `block-a-b`.
    let input = "```python #a.b\none\n```\n\n```python #a-b\ntwo\n```\n";
    let doc = weave_document(input, None, &global_ns()).unwrap();
    let html = doc.to_html(&HtmlOptions {
        standalone: false,
        title: None,
    });

    let ids: Vec<&str> = html
        .match_indices("<figure class=\"entangled-block\" id=\"")
        .map(|(i, m)| {
            let rest = &html[i + m.len()..];
            &rest[..rest.find('"').unwrap()]
        })
        .collect();
    assert_eq!(ids.len(), 2, "{html}");
    assert_ne!(ids[0], ids[1], "duplicate HTML id: {ids:?}");
}

#[test]
fn a_name_with_no_alphanumerics_still_gets_an_anchor() {
    use entangled::{weave_document, HtmlOptions};

    let input = "```python #-.-\none\n```\n\n```python #...\ntwo\n```\n";
    let doc = weave_document(input, None, &global_ns()).unwrap();
    let html = doc.to_html(&HtmlOptions {
        standalone: false,
        title: None,
    });

    let ids: Vec<&str> = html
        .match_indices("<figure class=\"entangled-block\" id=\"")
        .map(|(i, m)| {
            let rest = &html[i + m.len()..];
            &rest[..rest.find('"').unwrap()]
        })
        .collect();
    assert_eq!(ids.len(), 2, "{html}");
    assert_ne!(ids[0], ids[1], "duplicate HTML id: {ids:?}");
    assert!(ids.iter().all(|id| !id.is_empty()));
}

// --- F13: state files survive interleaved writers ----------------------------

#[test]
fn the_file_database_is_never_observed_half_written() {
    use entangled::io::FileDB;
    use entangled::io::FileData;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".entangled/filedb.json");

    let mut db = FileDB::new();
    for i in 0..200 {
        db.record(
            dir.path().join(format!("f{i}.py")),
            FileData::from_content(&"x".repeat(500), chrono::Utc::now()),
        );
    }
    db.save(&path).unwrap();

    // A reader racing a save must see one whole version or the other, never a
    // truncated file. With a plain `fs::write` this reads back as invalid JSON.
    let reader = {
        let path = path.clone();
        std::thread::spawn(move || {
            for _ in 0..200 {
                if path.exists() {
                    // A parse failure here is the corruption we are guarding against.
                    FileDB::load(&path).expect("database was observed half-written");
                }
            }
        })
    };
    for _ in 0..200 {
        db.save(&path).unwrap();
    }
    reader.join().unwrap();
}

#[test]
fn a_corrupt_file_database_is_quarantined_not_overwritten() {
    let dir = project(&[("doc.md", "```python #main file=out.py\nprint(1)\n```\n")]);
    let db_path = dir.path().join(".entangled/filedb.json");
    fs::create_dir_all(db_path.parent().unwrap()).unwrap();
    fs::write(&db_path, "{ this is not json").unwrap();

    let ctx = context(dir.path(), global_ns());
    assert!(ctx.filedb.is_empty());

    // The unparsable file is the only record of which files Entangled owns, so
    // it is moved aside rather than silently replaced.
    assert!(!db_path.exists());
    let quarantined = dir.path().join(".entangled/filedb.json.corrupt");
    assert!(quarantined.exists());
    assert_eq!(
        fs::read_to_string(quarantined).unwrap(),
        "{ this is not json"
    );
}
