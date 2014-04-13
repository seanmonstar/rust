// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An efficient hash map for node IDs

use collections::{HashMap, HashSet};
use hash::{Hasher, Hash};
use hash::fnv::FnvHasher;
use std::hash::{Hasher, Hash};
use std::io;
use syntax::ast;

pub type FnvHashMap<K, V> = HashMap<K, V, FnvHasher>;

pub type NodeMap<T> = FnvHashMap<ast::NodeId, T>;
pub type DefIdMap<T> = FnvHashMap<ast::DefId, T>;

pub type NodeSet = HashSet<ast::NodeId, FnvHasher>;
pub type DefIdSet = HashSet<ast::DefId, FnvHasher>;

// Hacks to get good names
pub mod FnvHashMap {
    use hash::Hash;
    use collections::HashMap;
    pub fn new<K: Hash<hash::fnv::FnvState> + TotalEq, V>() -> super::FnvHashMap<K, V> {
        HashMap::with_hasher(super::FnvHasher)
    }
}
pub mod NodeMap {
    pub fn new<T>() -> super::NodeMap<T> {
        super::FnvHashMap::new()
    }
}
pub mod DefIdMap {
    pub fn new<T>() -> super::DefIdMap<T> {
        super::FnvHashMap::new()
    }
}
pub mod NodeSet {
    use collections::HashSet;
    pub fn new() -> super::NodeSet {
        HashSet::with_hasher(super::FnvHasher)
    }
}
pub mod DefIdSet {
    use collections::HashSet;
    pub fn new() -> super::DefIdSet {
        HashSet::with_hasher(super::FnvHasher)
    }
}

