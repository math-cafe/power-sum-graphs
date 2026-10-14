//! Power-sum graphs.
//!
//! For `k >= 2` and `n >= 1`, the power-sum graph `G_k(n)` has vertices
//! `1..=n` and joins two distinct vertices whose sum is a perfect `k`-th
//! power. This crate holds everything posum computes about it without a
//! solver:
//!
//! - [`power`]: exact `k`-th roots;
//! - [`graph`]: `G_k(n)` by formula, with no stored edge;
//! - [`islands`]: the chain of blocking vertices, which gives the admissible
//!   islands and the bands certificate in `O(k)`;
//! - [`cycle`]: Hamiltonian cycle check and canonical order;
//! - [`bases`]: the bases encoding of a canonical cycle;
//! - [`mod@format`]: the text files of `doc/formats.md`;
//! - [`propagation`]: degree-two propagation, with the chain-closing rule;
//! - [`contraction`]: forced chains contracted to their ends, and back.

pub mod bases;
pub mod contraction;
pub mod cycle;
pub mod format;
pub mod graph;
pub mod islands;
pub mod power;
pub mod propagation;

pub use graph::{Graph, Vertex};
