//! Code block syntax style definitions.
//!
//! Supports multiple code block syntax styles commonly used in literate programming:
//! - `entangled-rs`: Native style with `python #name file=path`
//! - `pandoc`: Original entangled style with `{.python #name file=path}`
//! - `quarto`: Quarto/Jupyter style with `{python}` and `#|` comments
//! - `knitr`: RMarkdown style with `{python, label=name, file=path}`

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Code block syntax style.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum Style {
    /// Native entangled-rs style: ```python #main file=out.py
    #[default]
    EntangledRs,
    /// Original Pandoc/entangled style: ``` {.python #main file=out.py}
    Pandoc,
    /// Quarto style: ```{python} with #| label: main inside block
    Quarto,
    /// RMarkdown/knitr style: ```{python, label=main, file=out.py}
    Knitr,
}

impl Style {
    /// Detect style from file extension.
    ///
    /// Returns `Some(style)` if the extension indicates a specific style,
    /// `None` if the extension doesn't indicate a specific style (e.g., `.md`).
    pub fn from_extension(path: &Path) -> Option<Style> {
        let ext = path.extension()?.to_str()?;
        match ext.to_lowercase().as_str() {
            "qmd" => Some(Style::Quarto),
            "rmd" => Some(Style::Knitr),
            // .md files don't indicate a specific style
            _ => None,
        }
    }

    /// Determine the style for a document.
    ///
    /// Priority:
    /// 1. File extension (`.qmd` -> Quarto, `.Rmd` -> Knitr)
    /// 2. Configured default style
    pub fn for_document(path: Option<&Path>, config_default: Style) -> Style {
        if let Some(path) = path {
            if let Some(style) = Self::from_extension(path) {
                return style;
            }
        }
        config_default
    }

    /// Returns the style name as a static string.
    pub fn name(&self) -> &'static str {
        match self {
            Style::EntangledRs => "entangled-rs",
            Style::Pandoc => "pandoc",
            Style::Quarto => "quarto",
            Style::Knitr => "knitr",
        }
    }
}

impl std::fmt::Display for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::str::FromStr for Style {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "entangled-rs" | "entangledrs" | "entangled" => Ok(Style::EntangledRs),
            "pandoc" => Ok(Style::Pandoc),
            "quarto" => Ok(Style::Quarto),
            "knitr" | "rmarkdown" | "rmd" => Ok(Style::Knitr),
            _ => Err(format!(
                "Unknown style '{}'. Valid styles: entangled-rs, pandoc, quarto, knitr",
                s
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_extension_qmd() {
        assert_eq!(
            Style::from_extension(Path::new("doc.qmd")),
            Some(Style::Quarto)
        );
        assert_eq!(
            Style::from_extension(Path::new("path/to/doc.qmd")),
            Some(Style::Quarto)
        );
        assert_eq!(
            Style::from_extension(Path::new("DOC.QMD")),
            Some(Style::Quarto)
        );
    }

    #[test]
    fn test_from_extension_rmd() {
        assert_eq!(
            Style::from_extension(Path::new("doc.Rmd")),
            Some(Style::Knitr)
        );
        assert_eq!(
            Style::from_extension(Path::new("doc.rmd")),
            Some(Style::Knitr)
        );
        assert_eq!(
            Style::from_extension(Path::new("path/to/doc.Rmd")),
            Some(Style::Knitr)
        );
    }

    #[test]
    fn test_from_extension_md() {
        assert_eq!(Style::from_extension(Path::new("doc.md")), None);
        assert_eq!(Style::from_extension(Path::new("README.md")), None);
    }

    #[test]
    fn test_from_extension_other() {
        assert_eq!(Style::from_extension(Path::new("doc.txt")), None);
        assert_eq!(Style::from_extension(Path::new("doc")), None);
    }

    #[test]
    fn test_for_document_qmd() {
        let path = Path::new("doc.qmd");
        assert_eq!(
            Style::for_document(Some(path), Style::EntangledRs),
            Style::Quarto
        );
        assert_eq!(
            Style::for_document(Some(path), Style::Pandoc),
            Style::Quarto
        );
    }

    #[test]
    fn test_for_document_rmd() {
        let path = Path::new("doc.Rmd");
        assert_eq!(
            Style::for_document(Some(path), Style::EntangledRs),
            Style::Knitr
        );
        assert_eq!(Style::for_document(Some(path), Style::Quarto), Style::Knitr);
    }

    #[test]
    fn test_for_document_md_uses_default() {
        let path = Path::new("doc.md");
        assert_eq!(
            Style::for_document(Some(path), Style::EntangledRs),
            Style::EntangledRs
        );
        assert_eq!(
            Style::for_document(Some(path), Style::Pandoc),
            Style::Pandoc
        );
        assert_eq!(
            Style::for_document(Some(path), Style::Quarto),
            Style::Quarto
        );
    }

    #[test]
    fn test_for_document_no_path() {
        assert_eq!(
            Style::for_document(None, Style::EntangledRs),
            Style::EntangledRs
        );
        assert_eq!(Style::for_document(None, Style::Quarto), Style::Quarto);
    }

    #[test]
    fn test_from_str() {
        assert_eq!("entangled-rs".parse::<Style>().unwrap(), Style::EntangledRs);
        assert_eq!("pandoc".parse::<Style>().unwrap(), Style::Pandoc);
        assert_eq!("quarto".parse::<Style>().unwrap(), Style::Quarto);
        assert_eq!("knitr".parse::<Style>().unwrap(), Style::Knitr);
        assert_eq!("rmarkdown".parse::<Style>().unwrap(), Style::Knitr);
        assert!("invalid".parse::<Style>().is_err());
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Style::EntangledRs), "entangled-rs");
        assert_eq!(format!("{}", Style::Pandoc), "pandoc");
        assert_eq!(format!("{}", Style::Quarto), "quarto");
        assert_eq!(format!("{}", Style::Knitr), "knitr");
    }

    #[test]
    fn test_serde_roundtrip() {
        let styles = [
            Style::EntangledRs,
            Style::Pandoc,
            Style::Quarto,
            Style::Knitr,
        ];
        for style in styles {
            let json = serde_json::to_string(&style).unwrap();
            let parsed: Style = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, style);
        }
    }

    #[test]
    fn test_serde_kebab_case() {
        assert_eq!(
            serde_json::to_string(&Style::EntangledRs).unwrap(),
            "\"entangled-rs\""
        );
        assert_eq!(
            serde_json::from_str::<Style>("\"entangled-rs\"").unwrap(),
            Style::EntangledRs
        );
    }

    #[test]
    fn test_default() {
        assert_eq!(Style::default(), Style::EntangledRs);
    }
}
