use crate::models::{NamedQueryMatch, Themagraph};
use regex::{Regex, RegexBuilder};
use std::{collections::HashMap, sync::OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryNode {
    Value(String),
    And(Box<QueryNode>, Box<QueryNode>),
    Or(Box<QueryNode>, Box<QueryNode>),
    Not(Box<QueryNode>),
    Expansion(Box<QueryNode>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    pub message: String,
}

struct Parser {
    tokens: Vec<String>,
    position: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self {
            tokens: tokenize(input),
            position: 0,
        }
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.position).map(String::as_str)
    }

    fn consume(&mut self, expected: Option<&str>) -> Result<String, QueryError> {
        let token = self
            .tokens
            .get(self.position)
            .cloned()
            .ok_or_else(|| QueryError {
                message: "Unexpected end of input".to_owned(),
            })?;
        self.position += 1;
        if let Some(expected) = expected {
            if token != expected {
                return Err(QueryError {
                    message: format!("Expected '{expected}' but got '{token}'"),
                });
            }
        }
        Ok(token)
    }

    fn parse_value(&mut self) -> Result<QueryNode, QueryError> {
        match self.peek() {
            Some("(") => {
                self.consume(Some("("))?;
                let node = self.parse_or()?;
                self.consume(Some(")"))?;
                Ok(node)
            }
            Some(token) if token.starts_with("[[") && token.ends_with("]]") => {
                let token = self.consume(None)?;
                Ok(QueryNode::Value(normalize_link_text(&token)))
            }
            Some(token) => Err(QueryError {
                message: format!("Unexpected token: {token}"),
            }),
            None => Err(QueryError {
                message: "Unexpected end of input".to_owned(),
            }),
        }
    }

    fn parse_unary(&mut self) -> Result<QueryNode, QueryError> {
        match self.peek() {
            Some("!") => {
                self.consume(Some("!"))?;
                Ok(QueryNode::Not(Box::new(self.parse_unary()?)))
            }
            Some("*") => {
                self.consume(Some("*"))?;
                Ok(QueryNode::Expansion(Box::new(self.parse_unary()?)))
            }
            _ => self.parse_value(),
        }
    }

    fn parse_and(&mut self) -> Result<QueryNode, QueryError> {
        let mut node = self.parse_unary()?;
        while self.peek() == Some("&&") {
            self.consume(Some("&&"))?;
            node = QueryNode::And(Box::new(node), Box::new(self.parse_unary()?));
        }
        Ok(node)
    }

    fn parse_or(&mut self) -> Result<QueryNode, QueryError> {
        let mut node = self.parse_and()?;
        while self.peek() == Some("||") {
            self.consume(Some("||"))?;
            node = QueryNode::Or(Box::new(node), Box::new(self.parse_and()?));
        }
        Ok(node)
    }

    fn parse(mut self) -> Result<QueryNode, QueryError> {
        let ast = self.parse_or()?;
        if self.position < self.tokens.len() {
            return Err(QueryError {
                message: format!(
                    "Unexpected trailing tokens: {}",
                    self.tokens[self.position..].join(" ")
                ),
            });
        }
        Ok(ast)
    }
}

pub fn parse_query(input: &str) -> Result<QueryNode, QueryError> {
    Parser::new(input).parse()
}

pub fn filter_themagraphs<'a>(themagraphs: &'a [Themagraph], query: &str) -> Vec<&'a Themagraph> {
    let named_queries = named_query_definitions(themagraphs);
    match parse_query(query).map(|ast| {
        let expanded =
            expand_named_queries(&ast, &named_queries, &mut std::collections::HashSet::new());
        flatten_query(&expanded, themagraphs)
    }) {
        Ok(ast) => themagraphs
            .iter()
            .filter(|themagraph| evaluate_node(&ast, themagraph, themagraphs))
            .collect(),
        Err(_) => {
            let needle = query.trim().to_lowercase();
            if needle.is_empty() {
                return themagraphs.iter().collect();
            }
            themagraphs
                .iter()
                .filter(|themagraph| {
                    themagraph.body.to_lowercase().contains(&needle)
                        || themagraph
                            .links
                            .iter()
                            .any(|link| link.to_lowercase().contains(&needle))
                })
                .collect()
        }
    }
}

fn named_query_definitions(themagraphs: &[Themagraph]) -> HashMap<String, QueryNode> {
    themagraphs
        .iter()
        .filter(|themagraph| themagraph.links.len() == 1)
        .filter_map(|themagraph| {
            let body = themagraph.body.trim();
            if body.is_empty() {
                return None;
            }
            parse_query(body)
                .ok()
                .map(|definition| (themagraph.links[0].to_lowercase(), definition))
        })
        .collect()
}

pub fn named_query_names(themagraphs: &[Themagraph]) -> std::collections::HashSet<String> {
    themagraphs
        .iter()
        .filter(|themagraph| themagraph.links.len() == 1)
        .filter(|themagraph| {
            !themagraph.body.trim().is_empty() && parse_query(themagraph.body.trim()).is_ok()
        })
        .map(|themagraph| themagraph.links[0].clone())
        .collect()
}

pub fn named_queries_matching_link(themagraphs: &[Themagraph], link: &str) -> Vec<NamedQueryMatch> {
    let normalized_link = normalize_link_text(link);
    if normalized_link.is_empty() {
        return Vec::new();
    }
    let definitions = named_query_definitions(themagraphs);
    let synthetic = Themagraph {
        id: "reverse-query-link".to_owned(),
        body: String::new(),
        links: vec![normalized_link.clone()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    themagraphs
        .iter()
        .filter(|themagraph| themagraph.links.len() == 1)
        .filter_map(|themagraph| {
            let definition = definitions.get(&themagraph.links[0].to_lowercase())?;
            let expanded = expand_named_queries(
                definition,
                &definitions,
                &mut std::collections::HashSet::new(),
            );
            evaluate_node(&expanded, &synthetic, themagraphs).then(|| NamedQueryMatch {
                name: themagraph.links[0].clone(),
                query: themagraph.body.clone(),
            })
        })
        .collect()
}

fn expand_named_queries(
    node: &QueryNode,
    definitions: &HashMap<String, QueryNode>,
    expanding: &mut std::collections::HashSet<String>,
) -> QueryNode {
    match node {
        QueryNode::Value(value) => {
            let key = value.to_lowercase();
            let Some(definition) = definitions.get(&key) else {
                return node.clone();
            };
            if !expanding.insert(key.clone()) {
                return node.clone();
            }
            let expanded_definition = expand_named_queries(definition, definitions, expanding);
            expanding.remove(&key);
            QueryNode::Or(
                Box::new(QueryNode::Value(value.clone())),
                Box::new(expanded_definition),
            )
        }
        QueryNode::And(left, right) => QueryNode::And(
            Box::new(expand_named_queries(left, definitions, expanding)),
            Box::new(expand_named_queries(right, definitions, expanding)),
        ),
        QueryNode::Or(left, right) => QueryNode::Or(
            Box::new(expand_named_queries(left, definitions, expanding)),
            Box::new(expand_named_queries(right, definitions, expanding)),
        ),
        QueryNode::Not(inner) => QueryNode::Not(Box::new(expand_named_queries(
            inner,
            definitions,
            expanding,
        ))),
        QueryNode::Expansion(inner) => QueryNode::Expansion(Box::new(expand_named_queries(
            inner,
            definitions,
            expanding,
        ))),
    }
}

pub fn regex_filter_themagraphs<'a>(
    themagraphs: &'a [Themagraph],
    pattern: &str,
    case_insensitive: bool,
    target: Option<&str>,
) -> Result<Vec<&'a Themagraph>, QueryError> {
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .build()
        .map_err(|error| QueryError {
            message: format!("invalid regex pattern: {error}"),
        })?;

    Ok(themagraphs
        .iter()
        .filter(|themagraph| match target.unwrap_or("any") {
            "body" => regex.is_match(&themagraph.body),
            "links" => themagraph.links.iter().any(|link| regex.is_match(link)),
            "any" => {
                regex.is_match(&themagraph.body)
                    || themagraph.links.iter().any(|link| regex.is_match(link))
            }
            _ => false,
        })
        .collect())
}

pub fn regex_filter_tags<'a>(
    tags: &'a [String],
    pattern: &str,
    case_insensitive: bool,
) -> Result<Vec<&'a String>, QueryError> {
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .build()
        .map_err(|error| QueryError {
            message: format!("invalid regex pattern: {error}"),
        })?;

    Ok(tags.iter().filter(|tag| regex.is_match(tag)).collect())
}

fn tokenize(input: &str) -> Vec<String> {
    static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    TOKEN_RE
        .get_or_init(|| Regex::new(r"\[\[[^\]]+\]\]|\|\||&&|!|\*|\(|\)|\S+").expect("valid regex"))
        .find_iter(input)
        .map(|match_| match_.as_str().to_owned())
        .collect()
}

pub fn normalize_link_text(raw: &str) -> String {
    let trimmed = raw.trim();
    let unwrapped = trimmed
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
        .unwrap_or(trimmed);
    let before_alias = unwrapped.split('|').next().unwrap_or_default();
    let before_heading = before_alias.split('#').next().unwrap_or_default();
    before_heading.trim().to_owned()
}

pub fn extract_intralinks_from_text(text: &str) -> Vec<String> {
    static INTRALINK_RE: OnceLock<Regex> = OnceLock::new();
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for capture in INTRALINK_RE
        .get_or_init(|| Regex::new(r"\[\[([\s\S]+?)\]\]").expect("valid regex"))
        .captures_iter(text)
    {
        if let Some(content) = capture.get(1) {
            let normalized = normalize_link_text(content.as_str());
            let key = normalized.to_lowercase();
            if !normalized.is_empty() && seen.insert(key) {
                out.push(normalized);
            }
        }
    }
    out
}

pub fn merge_links(explicit_links: &[String], body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for link in explicit_links
        .iter()
        .map(|link| normalize_link_text(link))
        .chain(extract_intralinks_from_text(body))
    {
        let key = link.to_lowercase();
        if !link.is_empty() && seen.insert(key) {
            out.push(link);
        }
    }
    out
}

fn flatten_query(node: &QueryNode, themagraphs: &[Themagraph]) -> QueryNode {
    match node {
        QueryNode::Expansion(inner) => flatten_expansion(inner, themagraphs),
        QueryNode::And(left, right) => QueryNode::And(
            Box::new(flatten_query(left, themagraphs)),
            Box::new(flatten_query(right, themagraphs)),
        ),
        QueryNode::Or(left, right) => QueryNode::Or(
            Box::new(flatten_query(left, themagraphs)),
            Box::new(flatten_query(right, themagraphs)),
        ),
        QueryNode::Not(inner) => QueryNode::Not(Box::new(flatten_query(inner, themagraphs))),
        QueryNode::Value(value) => QueryNode::Value(value.clone()),
    }
}

fn flatten_expansion(node: &QueryNode, themagraphs: &[Themagraph]) -> QueryNode {
    let mut links = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for themagraph in themagraphs
        .iter()
        .filter(|themagraph| evaluate_node(node, themagraph, themagraphs))
    {
        for link in &themagraph.links {
            let key = link.to_lowercase();
            if seen.insert(key) {
                links.push(link.clone());
            }
        }
    }

    let mut nodes = links.into_iter().map(QueryNode::Value);
    let Some(first) = nodes.next() else {
        return QueryNode::Or(
            Box::new(QueryNode::Value("__never__".to_owned())),
            Box::new(QueryNode::Value("__never__".to_owned())),
        );
    };
    nodes.fold(first, |left, right| {
        QueryNode::Or(Box::new(left), Box::new(right))
    })
}

fn evaluate_node(
    node: &QueryNode,
    themagraph: &Themagraph,
    all_themagraphs: &[Themagraph],
) -> bool {
    match node {
        QueryNode::Value(value) => themagraph
            .links
            .iter()
            .any(|link| link.eq_ignore_ascii_case(value)),
        QueryNode::And(left, right) => {
            evaluate_node(left, themagraph, all_themagraphs)
                && evaluate_node(right, themagraph, all_themagraphs)
        }
        QueryNode::Or(left, right) => {
            evaluate_node(left, themagraph, all_themagraphs)
                || evaluate_node(right, themagraph, all_themagraphs)
        }
        QueryNode::Not(inner) => !evaluate_node(inner, themagraph, all_themagraphs),
        QueryNode::Expansion(inner) => evaluate_node(
            &flatten_expansion(inner, all_themagraphs),
            themagraph,
            all_themagraphs,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        extract_intralinks_from_text, filter_themagraphs, named_queries_matching_link,
        normalize_link_text, parse_query, regex_filter_tags, regex_filter_themagraphs,
    };
    use crate::models::Themagraph;
    use chrono::Utc;

    fn tg(id: &str, body: &str, links: &[&str]) -> Themagraph {
        Themagraph {
            id: id.to_owned(),
            body: body.to_owned(),
            links: links.iter().map(|value| (*value).to_owned()).collect(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn normalizes_links_like_rhizoidlib() {
        assert_eq!(normalize_link_text("[[foo|bar]]"), "foo");
        assert_eq!(normalize_link_text("[[foo#Heading]]"), "foo");
    }

    #[test]
    fn parses_boolean_query() {
        let ast = parse_query("[[alpha]] && ![[beta]]").expect("query should parse");
        assert!(matches!(ast, super::QueryNode::And(_, _)));
    }

    #[test]
    fn filters_with_and_query() {
        let themagraphs = vec![tg(
            "1",
            "body",
            &["programming rhizomatic server", "rhizomatic"],
        )];
        let matches = filter_themagraphs(
            &themagraphs,
            "[[programming rhizomatic server]] && [[rhizomatic]]",
        );
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn expands_named_queries_while_preserving_the_alias() {
        let themagraphs = vec![
            tg("definition", "[[craft]] && [[arts]]", &["craft arts"]),
            tg("underlying", "content", &["craft", "arts"]),
            tg("unrelated", "content", &["music"]),
        ];
        let matches = filter_themagraphs(&themagraphs, "[[craft arts]]");
        assert_eq!(
            matches
                .iter()
                .map(|themagraph| themagraph.id.as_str())
                .collect::<Vec<_>>(),
            vec!["definition", "underlying"]
        );
    }

    #[test]
    fn ignores_one_link_themagraphs_with_invalid_query_bodies() {
        let themagraphs = vec![
            tg("invalid", "not a query", &["alias"]),
            tg("alias", "content", &["alias"]),
        ];
        let matches = filter_themagraphs(&themagraphs, "[[alias]]");
        assert_eq!(
            matches
                .iter()
                .map(|themagraph| themagraph.id.as_str())
                .collect::<Vec<_>>(),
            vec!["invalid", "alias"]
        );
    }

    #[test]
    fn finds_named_queries_that_match_a_link() {
        let themagraphs = vec![
            tg("direct", "[[craft]]", &["craft alias"]),
            tg("compound", "[[craft]] && [[arts]]", &["craft and arts"]),
            tg("unrelated", "[[music]]", &["music alias"]),
        ];
        let matches = named_queries_matching_link(&themagraphs, "[[craft]]");
        assert_eq!(
            matches
                .into_iter()
                .map(|item| item.name)
                .collect::<Vec<_>>(),
            vec!["craft alias"]
        );
    }

    #[test]
    fn filters_with_expansion() {
        let themagraphs = vec![
            tg("1", "body", &["meta", "project"]),
            tg("2", "body", &["project", "rust"]),
            tg("3", "body", &["knowledge"]),
        ];
        let matches = filter_themagraphs(&themagraphs, "*[[meta]]");
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn falls_back_to_substring_search() {
        let themagraphs = vec![tg("1", "hello world", &["foo"])];
        let matches = filter_themagraphs(&themagraphs, "hello");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn extracts_body_links() {
        let links = extract_intralinks_from_text("see [[alpha]] and [[beta|Beta]]");
        assert_eq!(links, vec!["alpha".to_owned(), "beta".to_owned()]);
    }

    #[test]
    fn regex_filters_themagraph_body_and_links() {
        let themagraphs = vec![
            tg("1", "Body about rust", &["alpha"]),
            tg("2", "Elsewhere", &["rust-tag"]),
        ];
        assert_eq!(
            regex_filter_themagraphs(&themagraphs, "rust", true, Some("body"))
                .expect("regex should compile")
                .len(),
            1
        );
        assert_eq!(
            regex_filter_themagraphs(&themagraphs, "rust", true, Some("links"))
                .expect("regex should compile")
                .len(),
            1
        );
        assert_eq!(
            regex_filter_themagraphs(&themagraphs, "rust", true, Some("any"))
                .expect("regex should compile")
                .len(),
            2
        );
    }

    #[test]
    fn regex_filters_tags() {
        let tags = vec!["alpha".to_owned(), "rust-tag".to_owned()];
        let matches =
            regex_filter_tags(&tags, "^rust", true).expect("regex should compile successfully");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "rust-tag");
    }
}
