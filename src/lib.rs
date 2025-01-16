pub use feature::{FieldResult, FromFeature};

use gdal::errors::GdalError;
use thiserror::Error;

mod feature;

#[derive(Error, Debug, Clone)]
pub enum GdalTraitError {
    #[error("GDAL Error: {0}")]
    GdalError(#[from] GdalError),
    #[error("GDAL Trait error: Field is NULL")]
    NullField,
    #[error("GDAL Trait error: Invalid FieldValue: {0}")]
    InvalidFieldValue(String),
}
