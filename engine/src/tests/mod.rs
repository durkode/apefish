//! Crate-internal tests, grouped here rather than sitting alongside the
//! program modules in `src/`. These need direct access to `Position`
//! internals (and, to construct one at all, the private `ZobristRandoms`),
//! so they can't live in the top-level `tests/` directory - that's a
//! separate crate compiled only against this crate's public API.

mod zobrist_tests;
mod incremental_score_tests;
