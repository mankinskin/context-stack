use crate::*;
use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub struct PatternIndex {
    pub(crate) pattern_id: PatternId,
    pub(crate) sub_index: usize,
}
pub trait HasPatternId {
    fn pattern_id(&self) -> PatternId;
}
impl HasPatternId for PatternIndex {
    fn pattern_id(&self) -> PatternId {
        self.pattern_id
    }
}
impl HasPatternId for ChildLocation {
    fn pattern_id(&self) -> PatternId {
        self.pattern_id
    }
}
impl HasPatternId for SubLocation {
    fn pattern_id(&self) -> PatternId {
        self.pattern_id
    }
}
impl HasSubIndex for PatternIndex {
    fn sub_index(&self) -> usize {
        self.sub_index
    }
}
impl PatternIndex {
    pub(crate) fn new(
        pattern_id: PatternId,
        sub_index: usize,
    ) -> Self {
        Self {
            pattern_id,
            sub_index,
        }
    }
}

/// Storage for parent relationship of a token to a parent
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Parent {
    /// width of the parent
    pub(crate) width: TokenWidth,
    /// positions of token in parent patterns
    pub(crate) pattern_indices: HashSet<PatternIndex>,
}
impl Wide for Parent {
    fn width(&self) -> TokenWidth {
        self.width
    }
}
impl Parent {
    pub(crate) fn new(width: impl Into<TokenWidth>) -> Self {
        Self {
            width: width.into(),
            pattern_indices: Default::default(),
        }
    }
    pub fn pattern_indices(&self) -> &HashSet<PatternIndex> {
        &self.pattern_indices
    }
    pub(crate) fn get_width(&self) -> TokenWidth {
        self.width
    }
    pub(crate) fn add_pattern_index(
        &mut self,
        pattern_id: PatternId,
        sub_index: usize,
    ) {
        self.pattern_indices.insert(PatternIndex {
            pattern_id,
            sub_index,
        });
    }
    pub(crate) fn remove_pattern_index(
        &mut self,
        pattern_id: PatternId,
        sub_index: usize,
    ) {
        self.pattern_indices.remove(&PatternIndex {
            pattern_id,
            sub_index,
        });
    }
    pub(crate) fn exists_at_pos_in_pattern(
        &self,
        pattern_id: PatternId,
        sub_index: usize,
    ) -> bool {
        self.pattern_indices.contains(&PatternIndex {
            pattern_id,
            sub_index,
        })
    }
    pub(crate) fn get_index_at_pos(
        &self,
        p: usize,
    ) -> Option<PatternIndex> {
        self.pattern_indices
            .iter()
            .find(|i| i.sub_index == p)
            .cloned()
    }
    pub(crate) fn get_index_at_postfix_of(
        &self,
        v: &VertexData,
    ) -> Option<PatternIndex> {
        self.pattern_indices
            .iter()
            .find(|i| {
                v.expect_child_pattern(&i.pattern_id).len() == i.sub_index + 1
            })
            .cloned()
    }
}
#[allow(dead_code)]
impl Parent {
    pub(crate) fn any_pattern_index(&self) -> PatternIndex {
        *self.pattern_indices.iter().next().unwrap()
    }
    pub(crate) fn exists_at_pos(
        &self,
        p: usize,
    ) -> bool {
        self.pattern_indices.iter().any(|i| i.sub_index == p)
    }
    /// filter for pattern indices which occur at start of their patterns
    pub(crate) fn filter_pattern_indices_at_prefix(
        &self
    ) -> impl Iterator<Item = &PatternIndex> {
        self.pattern_indices
            .iter()
            .filter(move |pattern_index| pattern_index.sub_index == 0)
    }
    /// filter for pattern indices which occur at end of given patterns
    pub(crate) fn filter_pattern_indices_at_end_in_patterns<'a>(
        &'a self,
        patterns: &'a HashMap<PatternId, Pattern>,
    ) -> impl Iterator<Item = &'a PatternIndex> {
        self.pattern_indices.iter().filter(move |pattern_index| {
            pattern_index.sub_index + 1
                == patterns
                    .get(&pattern_index.pattern_id)
                    .expect("Pattern index not in patterns!")
                    .len()
        })
    }
    // filter for pattern indices which occur in given patterns
    //pub(crate) fn filter_pattern_indices_in_patterns<'a>(
    //    &'a self,
    //    patterns: &'a HashMap<PatternId, Pattern>,
    //) -> impl Iterator<Item = &'a (PatternId, usize)> {
    //    self.pattern_indices
    //        .iter()
    //        .filter(move |(pattern_index, sub_index)| {
    //            *sub_index
    //                == patterns
    //                    .get(pattern_index)
    //                    .expect("Pattern index not in patterns!")
    //        })
    //}
}
