#[derive(Default, PartialEq, Debug)]
pub struct TokenStrings {
    patterns: indexmap::IndexMap<String, Vec<Vec<String>>>,
}

impl TokenStrings {
    pub(crate) fn new() -> Self {
        Default::default()
    }
    pub(crate) fn from_nodes(
        nodes: impl IntoIterator<
            Item = (
                impl ToString,
                impl IntoIterator<Item = impl IntoIterator<Item = impl ToString>>,
            ),
        >
    ) -> Self {
        let mut g = Self::new();
        g.add_nodes(nodes);
        g
    }
    pub(crate) fn add_nodes(
        &mut self,
        node_patterns: impl IntoIterator<
            Item = (
                impl ToString,
                impl IntoIterator<Item = impl IntoIterator<Item = impl ToString>>,
            ),
        >,
    ) {
        self.patterns
            .extend(node_patterns.into_iter().map(|(name, node)| {
                (
                    name.to_string(),
                    node.into_iter()
                        .map(|p| p.into_iter().map(|p| p.to_string()).collect())
                        .collect(),
                )
            }));
    }
    //pub(crate) fn from_node(
    //    name: impl ToString,
    //    node: impl IntoIterator<Item = impl IntoIterator<Item = impl ToString>>,
    //) -> Self {
    //    let mut g = Self::new();
    //    g.add_node(name, node);
    //    g
    //}
    //pub(crate) fn add_node(
    //    &mut self,
    //    name: impl ToString,
    //    patterns: impl IntoIterator<Item = impl IntoIterator<Item = impl ToString>>,
    //) {
    //    let node = patterns
    //        .into_iter()
    //        .map(|p| p.into_iter().map(|p| p.to_string()).collect())
    //        .collect();
    //    self.patterns.insert(name.to_string(), node);
    //}
}
