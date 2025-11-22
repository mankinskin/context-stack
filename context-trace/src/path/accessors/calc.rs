use crate::{
    path::structs::rooted::{
        RangePath,
        root::PathRoot,
    },
    *,
};

pub trait CalcWidth: PathWidth {
    fn calc_width<G: HasGraph>(
        &self,
        trav: G,
    ) -> TokenWidth {
        self.path_width(&trav)
    }
}
impl<T: PathWidth> CalcWidth for T {}

//impl<Role: PathRole, Root: PathRoot> PathWidth for RootedRolePath<Role, Root>
//where
//    Self: RootedLeafToken<Role>,
//{
//    fn path_width<G: HasGraph>(
//        &self,
//        trav: G,
//    ) -> TokenWidth {
//        self.calc_offset(&trav) + self.rooted_leaf_token(&trav).width()
//    }
//}

pub trait CalcOffset {
    // TODO: Make offset side relative
    fn calc_offset<G: HasGraph>(
        &self,
        trav: G,
    ) -> TokenWidth;
}
impl<R: PathRole> CalcOffset for RolePath<R> {
    fn calc_offset<G: HasGraph>(
        &self,
        trav: G,
    ) -> TokenWidth {
        let graph = trav.graph();
        self.sub_path.path.iter().fold(TokenWidth(0), |acc, loc| {
            acc + loc.role_inner_width::<_, R>(&graph)
        })
    }
}
impl<Role: PathRole, Root: PathRoot> CalcOffset for RootedRolePath<Role, Root> {
    fn calc_offset<G: HasGraph>(
        &self,
        trav: G,
    ) -> TokenWidth {
        self.role_path.calc_offset(trav)
    }
}

pub trait PathWidth: CalcOffset + RootedPath {
    fn path_width<G: HasGraph>(
        &self,
        trav: G,
    ) -> TokenWidth;
}
impl<P: RangePath + CalcOffset + RootedPath> PathWidth for P {
    fn path_width<G: HasGraph>(
        &self,
        trav: G,
    ) -> TokenWidth {
        if self.role_root_child_index::<Start>()
            != self.role_root_child_index::<End>()
        {
            self.calc_offset(&trav)
                + self.role_rooted_leaf_token::<Start, _>(&trav).width()
                + self.role_rooted_leaf_token::<End, _>(&trav).width()
        } else {
            self.role_root_child_token::<Start, _>(&trav).width()
        }
    }
}

impl PathWidth for PatternEndPath {
    fn path_width<G: HasGraph>(
        &self,
        trav: G,
    ) -> TokenWidth {
        self.calc_offset(&trav)
            + self.role_rooted_leaf_token::<End, _>(&trav).width()
    }
}
