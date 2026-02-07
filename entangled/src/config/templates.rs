//! Built-in language templates.

use once_cell::sync::Lazy;

use super::language::{Comment, Language};

/// Built-in language configurations, lazily initialized.
static BUILTIN_LANGUAGES: Lazy<Vec<Language>> = Lazy::new(|| {
    vec![
        // C-style languages
        Language::new("c", Comment::line("//"))
            .with_identifiers(vec!["h".to_string()]),
        Language::new("cpp", Comment::line("//"))
            .with_identifiers(vec!["c++".to_string(), "cxx".to_string(), "hpp".to_string()]),
        Language::new("java", Comment::line("//")),
        Language::new("javascript", Comment::line("//"))
            .with_identifiers(vec!["js".to_string()]),
        Language::new("typescript", Comment::line("//"))
            .with_identifiers(vec!["ts".to_string()]),
        Language::new("rust", Comment::line("//"))
            .with_identifiers(vec!["rs".to_string()]),
        Language::new("go", Comment::line("//")),
        Language::new("swift", Comment::line("//")),
        Language::new("kotlin", Comment::line("//"))
            .with_identifiers(vec!["kt".to_string()]),
        Language::new("scala", Comment::line("//")),
        Language::new("csharp", Comment::line("//"))
            .with_identifiers(vec!["cs".to_string(), "c#".to_string()]),

        // Shell-style languages
        Language::new("python", Comment::line("#"))
            .with_identifiers(vec!["py".to_string(), "python3".to_string()]),
        Language::new("ruby", Comment::line("#"))
            .with_identifiers(vec!["rb".to_string()]),
        Language::new("perl", Comment::line("#"))
            .with_identifiers(vec!["pl".to_string()]),
        Language::new("bash", Comment::line("#"))
            .with_identifiers(vec!["sh".to_string(), "shell".to_string(), "zsh".to_string()]),
        Language::new("r", Comment::line("#")),
        Language::new("julia", Comment::line("#"))
            .with_identifiers(vec!["jl".to_string()]),
        Language::new("yaml", Comment::line("#"))
            .with_identifiers(vec!["yml".to_string()]),
        Language::new("toml", Comment::line("#")),
        Language::new("make", Comment::line("#"))
            .with_identifiers(vec!["makefile".to_string()]),
        Language::new("dockerfile", Comment::line("#"))
            .with_identifiers(vec!["docker".to_string()]),

        // Lisp-style languages
        Language::new("lisp", Comment::line(";"))
            .with_identifiers(vec!["cl".to_string(), "elisp".to_string()]),
        Language::new("scheme", Comment::line(";"))
            .with_identifiers(vec!["scm".to_string()]),
        Language::new("clojure", Comment::line(";"))
            .with_identifiers(vec!["clj".to_string(), "cljs".to_string()]),
        Language::new("racket", Comment::line(";"))
            .with_identifiers(vec!["rkt".to_string()]),

        // ML-style languages
        Language::new("haskell", Comment::line("--"))
            .with_identifiers(vec!["hs".to_string()]),
        Language::new("elm", Comment::line("--")),
        Language::new("ocaml", Comment::block("(*", "*)"))
            .with_identifiers(vec!["ml".to_string()]),
        Language::new("fsharp", Comment::line("//"))
            .with_identifiers(vec!["fs".to_string(), "f#".to_string()]),

        // Web languages
        Language::new("html", Comment::block("<!--", "-->"))
            .with_identifiers(vec!["htm".to_string()]),
        Language::new("css", Comment::block("/*", "*/")),
        Language::new("scss", Comment::line("//"))
            .with_identifiers(vec!["sass".to_string()]),

        // Config/Data languages
        Language::new("json", Comment::line("//")),
        Language::new("xml", Comment::block("<!--", "-->")),
        Language::new("sql", Comment::line("--")),

        // Other languages
        Language::new("lua", Comment::line("--")),
        Language::new("nim", Comment::line("#")),
        Language::new("zig", Comment::line("//")),
        Language::new("d", Comment::line("//")),
        Language::new("php", Comment::line("//")),
        Language::new("powershell", Comment::line("#"))
            .with_identifiers(vec!["ps1".to_string()]),
        Language::new("tex", Comment::line("%"))
            .with_identifiers(vec!["latex".to_string()]),
        Language::new("fortran", Comment::line("!"))
            .with_identifiers(vec!["f90".to_string(), "f95".to_string()]),
        Language::new("ada", Comment::line("--")),
        Language::new("vhdl", Comment::line("--")),
        Language::new("verilog", Comment::line("//"))
            .with_identifiers(vec!["v".to_string(), "sv".to_string()]),
    ]
});

/// Returns the list of built-in language configurations.
pub fn builtin_languages() -> &'static [Language] {
    &BUILTIN_LANGUAGES
}

/// Find a language by name or identifier.
pub fn find_language(identifier: &str) -> Option<Language> {
    builtin_languages()
        .iter()
        .find(|lang| lang.matches(identifier))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_python() {
        let lang = find_language("python").unwrap();
        assert_eq!(lang.name, "python");
        assert_eq!(lang.comment, Comment::line("#"));

        // Also find by alias
        let lang2 = find_language("py").unwrap();
        assert_eq!(lang2.name, "python");
    }

    #[test]
    fn test_find_rust() {
        let lang = find_language("rust").unwrap();
        assert_eq!(lang.name, "rust");
        assert_eq!(lang.comment, Comment::line("//"));

        let lang2 = find_language("rs").unwrap();
        assert_eq!(lang2.name, "rust");
    }

    #[test]
    fn test_find_html() {
        let lang = find_language("html").unwrap();
        assert_eq!(lang.comment, Comment::block("<!--", "-->"));
    }

    #[test]
    fn test_find_unknown() {
        assert!(find_language("unknown_language").is_none());
    }

    #[test]
    fn test_builtin_count() {
        let langs = builtin_languages();
        // We should have a good number of built-in languages
        assert!(langs.len() >= 30);
    }
}
