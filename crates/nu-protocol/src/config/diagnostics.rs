use super::prelude::*;
use crate as nu_protocol;

const DEFAULT_LANGUAGE_ID: &str = "nushell";
const DEFAULT_URI_SCHEME: &str = "file";

/// Language servers whose diagnostics are shown under the REPL input.
#[derive(Clone, Debug, Default, IntoValue, Serialize, Deserialize)]
pub struct DiagnosticsConfig {
    pub servers: Vec<DiagnosticServerConfig>,
}

/// One language server: how to start it and how it sees the input.
#[derive(Clone, Debug, IntoValue, Serialize, Deserialize)]
pub struct DiagnosticServerConfig {
    pub command: Vec<String>,
    pub language_id: String,
    pub uri_scheme: String,
}

fn command_words(value: &Value) -> Option<Vec<String>> {
    let words: Vec<_> = value
        .as_list()
        .ok()?
        .iter()
        .map(|word| word.as_str().map(str::to_string))
        .collect::<Result<_, _>>()
        .ok()?;
    (!words.is_empty()).then_some(words)
}

fn parse_server<'a>(
    value: &'a Value,
    path: &mut ConfigPath<'a>,
    errors: &mut ConfigErrors,
) -> Option<DiagnosticServerConfig> {
    let Value::Record { val: record, .. } = value else {
        errors.type_mismatch(path, Type::record(), value);
        return None;
    };
    let mut command = None;
    let mut language_id = DEFAULT_LANGUAGE_ID.to_string();
    let mut uri_scheme = DEFAULT_URI_SCHEME.to_string();
    for (col, val) in record.iter() {
        let path = &mut path.push(col);
        match col.as_str() {
            "command" => {
                command = command_words(val);
                if command.is_none() {
                    errors.type_mismatch(path, Type::custom("non-empty list<string>"), val);
                }
            }
            "language_id" => language_id.update(val, path, errors),
            "uri_scheme" => uri_scheme.update(val, path, errors),
            _ => errors.unknown_option(path, val),
        }
    }
    let Some(command) = command else {
        if !record.contains("command") {
            errors.missing_column(path, "command", value.span());
        }
        return None;
    };
    Some(DiagnosticServerConfig {
        command,
        language_id,
        uri_scheme,
    })
}

impl UpdateFromValue for DiagnosticsConfig {
    fn update<'a>(
        &mut self,
        value: &'a Value,
        path: &mut ConfigPath<'a>,
        errors: &mut ConfigErrors,
    ) {
        let Value::Record { val: record, .. } = value else {
            errors.type_mismatch(path, Type::record(), value);
            return;
        };

        for (col, val) in record.iter() {
            let path = &mut path.push(col);
            match col.as_str() {
                "servers" => {
                    let Value::List { vals, .. } = val else {
                        errors.type_mismatch(path, Type::list(Type::record()), val);
                        continue;
                    };
                    self.servers = vals
                        .iter()
                        .filter_map(|server| parse_server(server, path, errors))
                        .collect();
                }
                _ => errors.unknown_option(path, val),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, Span, record};

    fn updated_config(diagnostics: Value) -> (DiagnosticsConfig, bool) {
        let config = Config::default();
        let mut errors = ConfigErrors::new(&config);
        let mut diagnostics_config = DiagnosticsConfig::default();
        diagnostics_config.update(&diagnostics, &mut ConfigPath::new(), &mut errors);
        (diagnostics_config, errors.has_errors())
    }

    fn with_servers(servers: Vec<Value>) -> Value {
        Value::test_record(record! { "servers" => Value::list(servers, Span::test_data()) })
    }

    fn command_of(words: &[&str]) -> Value {
        Value::test_list(words.iter().map(|word| Value::test_string(*word)).collect())
    }

    #[test]
    fn unset_fields_use_the_defaults() {
        let server = Value::test_record(record! { "command" => command_of(&["nu-lint", "--lsp"]) });
        let (config, has_errors) = updated_config(with_servers(vec![server]));
        assert!(!has_errors);
        assert_eq!(config.servers[0].command, ["nu-lint", "--lsp"]);
        assert_eq!(config.servers[0].language_id, "nushell");
        assert_eq!(config.servers[0].uri_scheme, "file");
    }

    #[test]
    fn a_missing_command_is_reported() {
        let server = Value::test_record(record! { "language_id" => Value::test_string("bash") });
        let (config, has_errors) = updated_config(with_servers(vec![server]));
        assert!(has_errors);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn a_command_string_is_rejected_in_favor_of_a_list() {
        let server =
            Value::test_record(record! { "command" => Value::test_string("nu-lint --lsp") });
        let (config, has_errors) = updated_config(with_servers(vec![server]));
        assert!(has_errors);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn an_empty_command_is_rejected() {
        let server = Value::test_record(record! { "command" => command_of(&[]) });
        let (_, has_errors) = updated_config(with_servers(vec![server]));
        assert!(has_errors);
    }

    #[test]
    fn unknown_options_are_reported() {
        let server = Value::test_record(record! {
            "command" => command_of(&["nu-lint"]),
            "scheme" => Value::test_string("repl"),
        });
        let (_, has_errors) = updated_config(with_servers(vec![server]));
        assert!(has_errors);
    }
}
