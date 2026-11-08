//! The one integration-test binary of kh-ai: each file here is a module of it, so the shared test
//! host in `common` is compiled once and every helper in it has a caller.

mod common;

mod b5_state_warp_query;
mod bt_semantics;
mod c3_nodes;
mod c4_nodes;
mod cure_rules;
mod i1_warp_states;
mod real_support;
mod real_trees;
