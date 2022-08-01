use std::{
    collections::{HashMap, VecDeque},
    ops::Deref,
};

use cargo::core::compiler::{unit_graph::UnitDep, Unit};

pub trait UnitGraphExt {
    /// Breadth first search to find a parent-child relationship. I have no idea which search order will be fastest. This was just the order that popped into my head first.
    fn has_dependency(&self, maybe_parent: &Unit, maybe_child: &Unit) -> bool;
    fn find_by_name(&self, name: &'static str) -> Box<dyn Iterator<Item = &'_ Unit> + '_>;
}

impl UnitGraphExt for HashMap<Unit, Vec<UnitDep>> {
    fn has_dependency(&self, maybe_parent: &Unit, maybe_child: &Unit) -> bool {
        let mut haystack: VecDeque<&Unit> = Default::default();
        haystack.push_back(maybe_parent);
        while let Some(current) = haystack.pop_front() {
            if current == maybe_child {
                return true;
            }
            haystack.extend(self.get(current).unwrap().iter().map(|dep| &dep.unit))
        }
        false
    }
    fn find_by_name(&self, name: &'static str) -> Box<dyn Iterator<Item = &'_ Unit> + '_> {
        Box::new(
            self.keys()
                .filter(move |unit| (*unit).deref().pkg.name() == name),
        )
    }
}
