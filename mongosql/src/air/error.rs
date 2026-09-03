use crate::air::AggregationFunction;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors raised while constructing AIR nodes whose validity cannot be expressed
/// in the type system alone.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("$setWindowFields output field '{0}' requires a sortBy")]
    MissingSortBy(String),
    #[error("$setWindowFields range window requires exactly one ascending sortBy key")]
    InvalidRangeSortBy,
    #[error("aggregation function {0:?} is not yet supported as a window operator")]
    UnsupportedWindowFunction(AggregationFunction),
}
