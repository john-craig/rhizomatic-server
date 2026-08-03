use crate::{
    models::{RenderTemplateRequest, Themagraph},
    query::filter_themagraphs,
};
use regex::Regex;
use std::sync::OnceLock;

pub fn render_template(request: &RenderTemplateRequest, themagraphs: &[Themagraph]) -> String {
    static EXPRESSION_RE: OnceLock<Regex> = OnceLock::new();
    let expression_re = EXPRESSION_RE
        .get_or_init(|| Regex::new(r"<%([\s\S]*?)%>").expect("valid template expression regex"));
    let mut output = String::new();
    let mut last_index = 0;

    for capture in expression_re.captures_iter(&request.template) {
        let Some(full_match) = capture.get(0) else {
            continue;
        };
        output.push_str(&request.template[last_index..full_match.start()]);
        output.push_str(&evaluate_expression(
            capture
                .get(1)
                .map(|value| value.as_str())
                .unwrap_or_default(),
            request,
            themagraphs,
        ));
        last_index = full_match.end();
    }
    output.push_str(&request.template[last_index..]);
    output.trim_end().to_owned()
}

fn evaluate_expression(
    expression: &str,
    request: &RenderTemplateRequest,
    themagraphs: &[Themagraph],
) -> String {
    let expression = expression.trim();
    if expression.is_empty() {
        return String::new();
    }
    if expression == "$query" {
        return matching_bodies(&request.query, themagraphs);
    }
    if expression.eq_ignore_ascii_case("title()") {
        return request.title.clone();
    }
    if let Some(inner) = expression
        .strip_prefix("tasks(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return matching_tasks(&expand_template_query(inner, &request.query), themagraphs);
    }
    matching_bodies(
        &expand_template_query(expression, &request.query),
        themagraphs,
    )
}

fn expand_template_query(expression: &str, base_query: &str) -> String {
    let replacement = if base_query.trim().is_empty() {
        String::new()
    } else {
        format!("({})", base_query.trim())
    };
    expression.replace("$query", &replacement)
}

fn matching_bodies(query: &str, themagraphs: &[Themagraph]) -> String {
    filter_themagraphs(themagraphs, query)
        .into_iter()
        .map(|themagraph| themagraph.body.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn matching_tasks(query: &str, themagraphs: &[Themagraph]) -> String {
    filter_themagraphs(themagraphs, query)
        .into_iter()
        .flat_map(|themagraph| themagraph.body.lines())
        .filter(|line| is_task_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_task_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    (trimmed.starts_with("- [") || trimmed.starts_with("* [") || trimmed.starts_with("+ ["))
        && trimmed.as_bytes().get(2) == Some(&b'[')
        && trimmed.as_bytes().get(4) == Some(&b']')
}

#[cfg(test)]
mod tests {
    use super::render_template;
    use crate::models::{RenderTemplateRequest, Themagraph};
    use chrono::Utc;

    fn tg(id: &str, body: &str, links: &[&str]) -> Themagraph {
        Themagraph {
            id: id.to_owned(),
            body: body.to_owned(),
            links: links.iter().map(|link| (*link).to_owned()).collect(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn renders_title_query_and_tasks_expressions() {
        let themagraphs = vec![
            tg("one", "One body\n- [ ] One task", &["craft"]),
            tg("two", "Two body", &["music"]),
        ];
        let rendered = render_template(
            &RenderTemplateRequest {
                template: "# <% title() %>\n<% $query %>\n<% tasks($query) %>".to_owned(),
                query: "[[craft]]".to_owned(),
                title: "Craft Notes".to_owned(),
            },
            &themagraphs,
        );
        assert!(rendered.contains("# Craft Notes"));
        assert!(rendered.contains("One body"));
        assert!(rendered.contains("- [ ] One task"));
        assert!(!rendered.contains("Two body"));
    }
}
