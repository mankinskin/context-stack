//pub(crate) trait IntoFoldCtx<K: TraversalKind> {
//    fn start_search(
//        self,
//        trav: K::Trav,
//    ) -> SearchState<K>;
//}

//impl<K: TraversalKind, S: ToToken> IntoFoldCtx<K> for S {
//    fn start_search(
//        self,
//        trav: K::Trav,
//    ) -> SearchState<K> {
//        let start_index = self.to_child();
//        SearchState {
//            matches: SearchIterator::start_index(trav, start_index),
//            //max_width: start_index.width(),
//            start_index,
//            last_match: None,
//        }
//    }
//}
