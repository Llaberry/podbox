//! `--format '{{.Field}}'`, the part of docker's Go templates podbox answers.
//!
//! ⛔ **Selected by name, never by position.** `docs/conventions/code.md`. An
//! unknown field is refused and the known ones are listed, rather than
//! rendering as empty: a template that silently produces a blank column is a
//! wrong answer that looks like a right one.
//!
//! ⚠ **Right-sized on purpose.** This is not a Go template engine and does not
//! pretend to be: no pipelines, no functions, no conditionals. A template using
//! any of those is refused by name so a caller learns immediately, instead of
//! receiving something that parsed as a field name and was not one.

/// What went wrong with a template, for a message the caller can act on.
#[derive(Debug)]
pub enum Bad {
    Unterminated,
    Unsupported(String),
    UnknownField { got: String, known: Vec<String> },
}

impl std::fmt::Display for Bad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bad::Unterminated => write!(f, "the template has a `{{{{` that is never closed"),
            Bad::Unsupported(what) => write!(
                f,
                "{what:?} is not something podbox's --format understands. It \
                 takes `{{{{.Field}}}}` placeholders and literal text, and \
                 nothing else: no pipelines, no functions, no `table` prefix"
            ),
            Bad::UnknownField { got, known } => write!(
                f,
                "there is no field {got:?}. This verb has: {}",
                known.join(", ")
            ),
        }
    }
}

/// Expand `{{.Field}}` against `fields`, and the two escapes docker expands in
/// a format string before the template sees it.
pub fn render(template: &str, fields: &[(&str, String)]) -> Result<String, Bad> {
    let template = template.replace("\\t", "\t").replace("\\n", "\n");
    let known: Vec<String> = fields.iter().map(|(k, _)| format!(".{k}")).collect();
    let mut out = String::new();
    let mut rest = template.as_str();
    loop {
        let Some(at) = rest.find("{{") else {
            out.push_str(rest);
            return Ok(out);
        };
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let Some(end) = after.find("}}") else {
            return Err(Bad::Unterminated);
        };
        let expr = after[..end].trim();
        rest = &after[end + 2..];

        let Some(name) = expr.strip_prefix('.') else {
            return Err(Bad::Unsupported(expr.to_string()));
        };
        if name.contains(char::is_whitespace) || name.contains('|') || name.contains('.') {
            return Err(Bad::Unsupported(expr.to_string()));
        }
        match fields.iter().find(|(k, _)| *k == name) {
            Some((_, v)) => out.push_str(v),
            None => {
                return Err(Bad::UnknownField {
                    got: expr.to_string(),
                    known,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<(&'static str, String)> {
        vec![
            ("Repository", "alpine".into()),
            ("Tag", "latest".into()),
            ("Digest", "sha256:abc".into()),
        ]
    }

    #[test]
    fn the_acceptance_template_renders_the_digest_and_nothing_else() {
        // ⭐ TODO/milestones.md T-1102's acceptance is exactly this template.
        assert_eq!(render("{{.Digest}}", &fields()).unwrap(), "sha256:abc");
    }

    #[test]
    fn literal_text_and_several_fields_survive_together() {
        assert_eq!(
            render("{{.Repository}}:{{.Tag}} -> {{.Digest}}", &fields()).unwrap(),
            "alpine:latest -> sha256:abc"
        );
        assert_eq!(
            render("no fields here", &fields()).unwrap(),
            "no fields here"
        );
    }

    #[test]
    fn the_escapes_docker_expands_are_expanded() {
        assert_eq!(
            render("{{.Repository}}\\t{{.Tag}}", &fields()).unwrap(),
            "alpine\tlatest"
        );
    }

    #[test]
    fn an_unknown_field_is_refused_and_lists_the_known_ones() {
        // ⛔ Never a blank. A silently empty column is a wrong answer that
        // looks authoritative.
        let e = render("{{.Nope}}", &fields()).unwrap_err();
        let text = format!("{e}");
        assert!(text.contains(".Nope"), "{text}");
        assert!(text.contains(".Repository"), "{text}");
    }

    #[test]
    fn a_template_feature_podbox_does_not_have_is_refused_rather_than_guessed() {
        for bad in [
            "{{json .}}",
            "{{.Repository | upper}}",
            "{{if .Tag}}x{{end}}",
        ] {
            assert!(render(bad, &fields()).is_err(), "{bad} was accepted");
        }
        assert!(matches!(
            render("{{.Tag", &fields()).unwrap_err(),
            Bad::Unterminated
        ));
    }

    #[test]
    fn whitespace_inside_the_braces_is_the_go_spelling_and_is_accepted() {
        assert_eq!(render("{{ .Tag }}", &fields()).unwrap(), "latest");
    }
}
