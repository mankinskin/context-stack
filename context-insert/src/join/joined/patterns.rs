use crate::{
    interval::partition::{
        delta::PatternSubDeltas,
        info::{
            PartitionInfo,
            border::perfect::{
                BorderPerfect,
                SinglePerfect,
            },
            range::role::RangeRole,
        },
    },
    join::{
        context::{
            node::context::NodeJoinCtx,
            pattern::borders::JoinBorders,
        },
        partition::{
            Join,
            info::{
                JoinPartitionInfo,
                pattern_info::JoinPatternInfo,
            },
        },
    },
};
use context_trace::*;

#[derive(Debug)]
pub struct JoinedPatterns<R: RangeRole> {
    pub patterns: Vec<Pattern>,
    pub perfect: R::Perfect,
    pub range: Option<R::PatternRange>,
    pub delta: PatternSubDeltas,
}

impl<'a, R: RangeRole<Mode = Join> + 'a> JoinedPatterns<R>
where
    R::Borders: JoinBorders<R>,
{
    pub fn from_partition_info<'b>(
        info: JoinPartitionInfo<R>,
        ctx: &'b mut NodeJoinCtx<'a>,
    ) -> Self {
        // assert: no complete perfect token
        // todo: index inner ranges and get token splits
        //
        // index inner range
        // cases:
        // - (token, inner, token)
        // - (token, inner),
        // - (inner, token),
        // - (token, token),
        // - token: not possible, handled earlier
        let range = if let SinglePerfect(Some(pid)) = info.perfect.complete() {
            Some(info.patterns[&pid].range.clone())
        } else {
            None
        };
        let perfect = info.perfect.clone();
        let (delta, patterns) = PartitionInfo::from(info)
            .patterns
            .into_iter()
            .map(|(pid, pinfo): (PatternId, JoinPatternInfo<_>)| {
                ((pid, pinfo.delta), pinfo.join_pattern(ctx, &pid))
            })
            .unzip();
        Self {
            patterns,
            perfect,
            range,
            delta,
        }
    }
    //pub fn to_joined_partition(
    //    self,
    //    ctx: &'b mut NodeJoinCtx<'a>,
    //) -> JoinedPartition<R> {
    //    JoinedPartition::from_joined_patterns(self, ctx)
    //}
}
//#[derive(Debug)]
//pub enum JoinedPattern {
//    Trigram([Token; 3]),
//    Bigram([Token; 2]),
//}
//impl From<BorderChildren<JoinedRangeInfoKind>> for JoinedPattern {
//    fn from(borders: BorderChildren<JoinedRangeInfoKind>) -> Self {
//        match borders {
//            BorderChildren::Infix(left, right, None) =>
//                JoinedPattern::Bigram([left, right]),
//            BorderChildren::Infix(left, right, Some(inner)) =>
//                JoinedPattern::Trigram([left, inner, right]),
//            BorderChildren::Prefix(inner, right) =>
//                JoinedPattern::Bigram([inner, right]),
//            BorderChildren::Postfix(left, inner) =>
//                JoinedPattern::Bigram([left, inner]),
//        }
//    }
//}
//impl<'p> Borrow<[Token]> for &'p JoinedPattern {
//    fn borrow(&self) -> &[Token] {
//        match self {
//            JoinedPattern::Trigram(p) => p.borrow(),
//            JoinedPattern::Bigram(p) => p.borrow(),
//        }
//    }
//}
//impl<'p> IntoIterator for &'p JoinedPattern {
//    type Item = &'p Token;
//    type IntoIter = std::slice::Iter<'p, Token>;
//    fn into_iter(self) -> Self::IntoIter {
//        match self {
//            JoinedPattern::Trigram(p) => p.into_iter(),
//            JoinedPattern::Bigram(p) => p.into_iter(),
//        }
//    }
//}
//impl Deref for JoinedPattern {
//    type Target = [Token];
//    fn deref(&self) -> &Self::Target {
//        match self {
//            Self::Trigram(p) => p,
//            Self::Bigram(p) => p,
//        }
//    }
//}
//impl<'p> From<&[Token]> for JoinedPattern {
//    fn from(value: &[Token]) -> Self {
//        JoinedPattern::Bigram(
//            value.try_into().expect("unmerged partition without inner range not a bigram")
//        )
//    }
//}
