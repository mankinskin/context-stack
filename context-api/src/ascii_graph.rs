use ascii_dag::Graph;
use std::collections::{
    BTreeMap,
    BTreeSet,
};

#[derive(Debug, Clone)]
pub struct AsciiRule<'a> {
    pub parent: &'a str,
    pub patterns: &'a [&'a [&'a str]],
}

#[derive(Debug, Clone)]
pub struct AsciiOwnedRule {
    pub parent: String,
    pub patterns: Vec<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum AsciiRenderError {
    #[error("grammar graph has a cycle")]
    CyclicGraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiRenderMode {
    Grammar,
    Dag,
}

pub fn render_layered_ascii(rules: &[AsciiRule<'_>]) -> Result<String, AsciiRenderError> {
    let owned_rules: Vec<AsciiOwnedRule> = rules
        .iter()
        .map(|r| AsciiOwnedRule {
            parent: r.parent.to_string(),
            patterns: r
                .patterns
                .iter()
                .map(|p| p.iter().map(|s| (*s).to_string()).collect())
                .collect(),
        })
        .collect();
    render_layered_ascii_owned(&owned_rules)
}

pub fn render_layered_ascii_owned(
    rules: &[AsciiOwnedRule],
) -> Result<String, AsciiRenderError> {
    render_layered_ascii_owned_with_mode(rules, AsciiRenderMode::Grammar)
}

pub fn render_layered_ascii_owned_with_mode(
    rules: &[AsciiOwnedRule],
    mode: AsciiRenderMode,
) -> Result<String, AsciiRenderError> {
    let mut all_labels = BTreeSet::<String>::new();
    let mut edges_by_label = BTreeSet::<(String, String)>::new();

    for rule in rules {
        all_labels.insert(rule.parent.clone());
        for pat in &rule.patterns {
            for child in pat {
                all_labels.insert(child.clone());
                edges_by_label.insert((rule.parent.clone(), child.clone()));
            }
        }
    }

    let mut id_by_label = BTreeMap::<String, usize>::new();
    for (id, label) in all_labels.into_iter().enumerate() {
        id_by_label.insert(label, id);
    }

    let mut graph = Graph::new();
    for (label, id) in &id_by_label {
        graph.add_node(*id, label);
    }

    for (parent, child) in &edges_by_label {
        let from_id = *id_by_label
            .get(parent)
            .expect("parent id must exist while rendering");
        let to_id = *id_by_label
            .get(child)
            .expect("child id must exist while rendering");
        graph.add_edge(from_id, to_id, None);
    }

    if graph.has_cycle() {
        return Err(AsciiRenderError::CyclicGraph);
    }

    match mode {
        AsciiRenderMode::Grammar => {
            let mut patterns_by_parent =
                BTreeMap::<String, Vec<Vec<String>>>::new();
            for rule in rules {
                patterns_by_parent
                    .entry(rule.parent.clone())
                    .or_default()
                    .extend(rule.patterns.clone());
            }

            let mut labels: Vec<String> = patterns_by_parent
                .keys()
                .cloned()
                .collect();
            labels.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));

            let mut leaf_labels: Vec<String> = id_by_label
                .keys()
                .filter(|label| !patterns_by_parent.contains_key(*label))
                .cloned()
                .collect();
            leaf_labels.sort();

            let token_width = labels
                .iter()
                .map(|label| label.len() + 2)
                .max()
                .unwrap_or(0);

            let mut out = String::new();
            out.push_str("Grammar layout (aligned)\n");
            if !leaf_labels.is_empty() {
                out.push_str(&format!("Atoms: {}\n", leaf_labels.join(", ")));
            }
            out.push('\n');
            for label in labels {
                let mut patterns =
                    patterns_by_parent.get(&label).cloned().unwrap_or_default();
                patterns.sort();
                if patterns.is_empty() {
                    out.push_str(&format!(
                        "{:<token_width$} -> (leaf)\n",
                        format!("({})", label),
                        token_width = token_width
                    ));
                } else {
                    let rendered_patterns = patterns
                        .iter()
                        .map(|pattern| format!("[{}]", pattern.join(", ")))
                        .collect::<Vec<_>>();
                    if rendered_patterns.len() == 1 {
                        out.push_str(&format!(
                            "{:<token_width$} -> {}\n",
                            format!("({})", label),
                            rendered_patterns[0]
                            ,
                            token_width = token_width
                        ));
                    } else {
                        out.push_str(&format!(
                            "{:<token_width$} -> {{ {} }}\n",
                            format!("({})", label),
                            rendered_patterns.join(", "),
                            token_width = token_width
                        ));
                    }
                }
            }
            Ok(out)
        },
        AsciiRenderMode::Dag => Ok(graph.render()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_layers_with_longest_on_top() {
        let rules = [
            AsciiRule {
                parent: "abc",
                patterns: &[&["ab", "c"]],
            },
            AsciiRule {
                parent: "ab",
                patterns: &[&["a", "b"]],
            },
        ];

        let ascii = render_layered_ascii(&rules).expect("should render");
        assert!(ascii.contains("abc"));
        assert!(ascii.contains("ab"));
        assert!(ascii.contains("c"));
        assert!(ascii.contains("a"));
        assert!(ascii.contains("b"));
        assert!(ascii.contains("↓") || ascii.contains("->"));
    }

    #[test]
    fn rejects_cyclic_grammar() {
        let cyclic = [
            AsciiRule {
                parent: "ab",
                patterns: &[&["ba"]],
            },
            AsciiRule {
                parent: "ba",
                patterns: &[&["ab"]],
            },
        ];

        let err = render_layered_ascii(&cyclic).expect_err("must reject cycle");
        assert!(matches!(err, AsciiRenderError::CyclicGraph));
    }

    #[test]
    fn renders_default_grammar_layout_with_patterns() {
        let rules = [
            AsciiRule {
                parent: "abc",
                patterns: &[&["ab", "c"]],
            },
            AsciiRule {
                parent: "abab",
                patterns: &[&["ab", "ab"], &["aba", "b"]],
            },
            AsciiRule {
                parent: "ab",
                patterns: &[&["a", "b"]],
            },
            AsciiRule {
                parent: "aba",
                patterns: &[&["ab", "a"]],
            },
        ];

        let simple = render_layered_ascii(&rules)
            .expect("should render default grammar layout");

        assert!(simple.contains("Grammar layout (aligned)"));
        assert!(simple.contains("Atoms: a, b, c"));
        assert!(simple.contains("(abc)"));
        assert!(simple.contains("[ab, c]"));
        assert!(simple.contains("(abab)"));
        assert!(simple.contains("{ [ab, ab], [aba, b] }"));
        assert!(simple.contains("(ab)"));
        assert!(simple.contains("[a, b]"));
        assert!(simple.contains("(aba)"));
        assert!(simple.contains("[ab, a]"));
    }

    #[test]
    fn renders_ascii_dag_when_selected() {
        let rules = [
            AsciiRule {
                parent: "abc",
                patterns: &[&["ab", "c"]],
            },
            AsciiRule {
                parent: "ab",
                patterns: &[&["a", "b"]],
            },
        ];

        let owned_rules: Vec<AsciiOwnedRule> = rules
            .iter()
            .map(|r| AsciiOwnedRule {
                parent: r.parent.to_string(),
                patterns: r
                    .patterns
                    .iter()
                    .map(|p| p.iter().map(|s| (*s).to_string()).collect())
                    .collect(),
            })
            .collect();

        let dag = render_layered_ascii_owned_with_mode(
            &owned_rules,
            AsciiRenderMode::Dag,
        )
        .expect("should render dag mode");
        assert!(dag.contains("abc"));
        assert!(dag.contains("ab"));
        assert!(dag.contains("a"));
        assert!(dag.contains("b"));
    }
}
