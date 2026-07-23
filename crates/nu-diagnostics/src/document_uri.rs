use lsp_types::Uri;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentScheme {
    WorkspaceFile,
    Virtual { scheme: String },
}

impl DocumentScheme {
    #[must_use]
    pub fn from_scheme_name(scheme_name: &str) -> Self {
        match scheme_name {
            "file" => Self::WorkspaceFile,
            other => Self::Virtual {
                scheme: other.to_string(),
            },
        }
    }
}

fn language_file_extension(language_id: &str) -> &str {
    match language_id {
        "bash" | "shellscript" => "sh",
        "nushell" | "nu" => "nu",
        "python" => "py",
        "rust" => "rs",
        "javascript" => "js",
        "typescript" => "ts",
        "ruby" => "rb",
        "perl" => "pl",
        other => other,
    }
}

pub(crate) fn repl_document_uri(
    scheme: &DocumentScheme,
    language_id: &str,
    root_directory: &Path,
) -> Option<Uri> {
    let extension = language_file_extension(language_id);
    match scheme {
        DocumentScheme::WorkspaceFile => {
            file_uri(&root_directory.join(format!(".reedline-repl.{extension}")))
        }
        DocumentScheme::Virtual { scheme } => format!("{scheme}:///repl.{extension}").parse().ok(),
    }
}

pub(crate) fn file_uri(path: &Path) -> Option<Uri> {
    url::Url::from_file_path(path).ok()?.as_str().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_files_live_in_the_root_directory() {
        let document_uri =
            repl_document_uri(&DocumentScheme::WorkspaceFile, "bash", Path::new("/work"));
        assert_eq!(
            document_uri.map(|uri| uri.as_str().to_string()),
            Some("file:///work/.reedline-repl.sh".to_string())
        );
    }

    #[test]
    fn virtual_documents_use_their_scheme() {
        let document_uri = repl_document_uri(
            &DocumentScheme::from_scheme_name("repl"),
            "nushell",
            Path::new("/work"),
        );
        assert_eq!(
            document_uri.map(|uri| uri.as_str().to_string()),
            Some("repl:///repl.nu".to_string())
        );
    }

    #[test]
    fn relative_roots_have_no_file_uri() {
        let document_uri =
            repl_document_uri(&DocumentScheme::WorkspaceFile, "nushell", Path::new("work"));
        assert!(document_uri.is_none());
    }
}
