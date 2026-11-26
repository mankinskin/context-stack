//! Pattern replacement operations

use itertools::Itertools;

use crate::graph::{
    getters::vertex::VertexSet,
    kind::GraphKind,
    vertex::{
        has_vertex_index::HasVertexIndex,
        location::pattern::IntoPatternLocation,
        parent::PatternIndex,
        pattern::{
            IntoPattern,
            Pattern,
            pattern_range::PatternRangeIndex,
            replace_in_pattern,
        },
    },
};

impl<G> crate::graph::Hypergraph<G>
where
    G: GraphKind,
{
    pub fn replace_pattern(
        &self,
        location: impl IntoPatternLocation,
        replace: impl IntoPattern,
    ) {
        let location = location.into_pattern_location();
        let parent = location.parent;
        let parent_index = parent.vertex_index();
        let pat = location.pattern_id;
        let replace = replace.into_pattern();

        self.add_pattern_parent(parent, &replace, pat, 0);

        let replaced = self
            .with_vertex_mut(parent, |vertex| {
                let pattern = vertex.expect_child_pattern_mut(&pat);
                let replaced = pattern.clone();
                *pattern = replace;
                vertex.validate();
                replaced
            })
            .expect("Parent vertex should exist");

        replaced.iter().enumerate().for_each(|(pos, c)| {
            self.with_vertex_mut(c.vertex_index(), |node| {
                node.remove_parent_index(parent_index, pat, pos);
            })
            .expect("Child vertex should exist");
        });
        self.validate_expansion(parent_index);
    }

    pub fn replace_in_pattern(
        &self,
        location: impl IntoPatternLocation,
        range: impl PatternRangeIndex,
        replace: impl IntoPattern,
    ) {
        let replace = replace.into_pattern();
        let location = location.into_pattern_location();
        let parent = location.parent;
        let parent_index = parent.vertex_index();
        let pat = location.pattern_id;

        let (replaced, start, new_end, rem) = self
            .with_vertex_mut(parent, |vertex| {
                let pattern = vertex.expect_child_pattern_mut(&pat);
                let start = range.clone().next().unwrap();
                let new_end = start + replace.len();
                let replaced = replace_in_pattern(
                    pattern.as_vec_mut(),
                    range.clone(),
                    replace.clone(),
                );
                let rem =
                    pattern.iter().skip(new_end).cloned().collect::<Pattern>();
                vertex.validate();
                (replaced, start, new_end, rem)
            })
            .expect("Parent vertex should exist");

        let old_end = start + replaced.len();
        range.clone().zip(replaced).for_each(|(pos, c)| {
            self.with_vertex_mut(c.vertex_index(), |node| {
                node.remove_parent_index(parent_index, pat, pos);
            })
            .expect("Child vertex should exist");
        });

        // shift sub indices in parent links of children in remaining pattern
        for c in rem.into_iter().unique() {
            self.with_vertex_mut(c.vertex_index(), |node| {
                let indices =
                    &mut node.expect_parent_mut(parent_index).pattern_indices;
                *indices = indices
                    .drain()
                    .filter(|i| {
                        i.pattern_id != pat
                            || !range.clone().contains(&i.sub_index)
                    })
                    .map(|i| {
                        if i.pattern_id == pat && i.sub_index >= old_end {
                            PatternIndex::new(
                                i.pattern_id,
                                i.sub_index - old_end + new_end,
                            )
                        } else {
                            i
                        }
                    })
                    .collect();
                if indices.is_empty() {
                    node.remove_parent(parent_index);
                }
            })
            .expect("Child vertex should exist");
        }
        self.add_pattern_parent(parent, replace, pat, start);
        self.validate_expansion(parent_index);
    }
}
