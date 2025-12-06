use std::{
    borrow::Borrow,
    num::NonZeroUsize,
};

use crate::*;
use context_trace::*;

use pretty_assertions::assert_eq;

pub mod cases;
pub mod env;
pub mod insert;
pub mod interval;
pub mod test_case;

pub(crate) fn pattern_from_widths(
    widths: impl IntoIterator<Item = usize>
) -> Pattern {
    widths
        .into_iter()
        .enumerate()
        .map(|(i, w)| Token::new(VertexIndex(i), w))
        .collect()
}

#[test]
fn atom_pos_split() {
    // No graph available in this test - uses pattern_from_widths helper
    let _tracing = context_trace::init_test_tracing!();
    let pattern = pattern_from_widths([1, 1, 3, 1, 1]);
    let width = pattern_width(&pattern);
    assert_eq!(
        TraceBack::trace_child_pos(
            pattern.borrow() as &[Token],
            NonZeroUsize::new(2).unwrap(),
        ),
        Some((2, None).into()),
    );
    assert_eq!(
        TraceFront::trace_child_pos(
            pattern.borrow() as &[Token],
            NonZeroUsize::new(*width - 2).unwrap(),
        ),
        Some((2, None).into()),
    );
    assert_eq!(
        TraceFront::trace_child_pos(
            pattern.borrow() as &[Token],
            NonZeroUsize::new(*width - 4).unwrap(),
        ),
        Some((2, NonZeroUsize::new(1)).into()),
    );
}

//#[macro_export]
//macro_rules! insert_patterns2 {
//    ($graph:ident,
//        $(
//            $name1:ident => [
//                $($pat1:ident),*
//                $([$($pat2:expr),*]),*
//                $(,)?
//            ]
//            $(
//                ($name2:ident, $idname:ident) => [
//                    $($pat3:ident),*
//                    $([$($pat4:ident),*]),*
//                    $(,)?
//                ]
//            )?
//        ),*
//        $(,)?
//    ) => {
//
//        $(
//            let $name1: Token = $graph.insert_pattern([$($pat1),*]);
//            let $name1: Token = $graph.insert_patterns(vec![$(vec![$($pat2),*]),*] as Vec<context_trace::graph::vertex::pattern::Pattern>);
//            $(
//                let ($name2, $idname): (Token, _) = $graph.graph_mut().insert_pattern_with_id([$($pat3),*]);
//                let $idname = $idname.unwrap();
//            )?
//            $(let ($name2, $idname): (Token, _) = $graph.graph_mut().insert_patterns_with_ids([$(vec![$($pat4),*]),*]))?
//        )*
//    };
//}
//

#[macro_export]
macro_rules! nz {
    ($x:expr) => {
        std::num::NonZeroUsize::new($x).unwrap()
    };
}
#[macro_export]
macro_rules! build_split_cache {
    (
        $root_mode:expr,
        $(
            $entry_root:ident => {
                $(
                    $pos:expr => {
                        top: [$($top:ident: $top_pos:expr),*$(,)?],
                        splits: [$($pid:expr => ($sub:expr, $inner:expr)),*$(,)?]
                    }
                ),*$(,)?
            }
        ),*
        $(,)?
    ) => {
        $crate::SplitCache {
            root_mode: $root_mode,
            entries: context_trace::HashMap::from_iter([
                $(
                    (
                        $entry_root.index,
                        $crate::SplitVertexCache {
                            positions: BTreeMap::from_iter([
                                $(
                                    (
                                        nz!($pos),
                                        $crate::SplitPositionCache {
                                            top: context_trace::HashSet::from_iter([
                                                $(
                                                    $crate::PosKey {
                                                        index: $top.to_owned(),
                                                        pos: nz!($top_pos),
                                                    }
                                                ),*
                                            ]),
                                            pattern_splits: context_trace::HashMap::from_iter([
                                                $(
                                                    (
                                                        $pid.to_owned(),
                                                        $crate::TokenTracePos {
                                                            inner_offset: $inner,
                                                            sub_index: $sub,
                                                        }
                                                    )
                                                ),*
                                            ])
                                        }
                                    )
                                ),*
                            ])
                        }
                    )
                ),*
            ])
        }
    };
}
