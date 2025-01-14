pub use feature::FeatureTrait;

use gdal::errors::GdalError;
use gdal::vector::Geometry;
use thiserror::Error;

mod dataset;
mod feature;

#[derive(Error, Debug, Clone)]
pub enum GdalTraitError {
    #[error("GDAL Error: {0}")]
    GdalError(#[from] GdalError),
    #[error("GDAL Trait error: Field is NULL")]
    NullField,
    #[error("GDAL Trait error: Invalid FieldValue: {0}")]
    InvalidFieldValue(String),
    #[error("GDAL Trait error: Invalid Geometry: {0:?}")]
    InvalidGeometry(Geometry),
}
