use std::error::Error;

use chrono::{DateTime, FixedOffset, NaiveDate};
use gdal::errors::GdalError;
use gdal::vector::{Feature, FieldValue, Geometry, Layer, LayerAccess};
use paste::paste;

use crate::GdalTraitError;

/// Retrieval result of a field from a layer.
///
/// Some(...)   - Success, returns value.
/// Null        - Success, though the field is a NULL value.
/// Error(E)    - Failure, returns the responsible error.
#[derive(Debug, Clone)]
pub enum FieldResult<E: Error + Clone> {
    Some(FieldValue),
    Null,
    Error(E),
}

impl From<Result<Option<FieldValue>, GdalError>> for FieldResult<GdalTraitError> {
    fn from(value: Result<Option<FieldValue>, GdalError>) -> Self {
        match value {
            Ok(field) => match field {
                Some(f) => FieldResult::Some(f),
                None => FieldResult::Null,
            },
            Err(e) => FieldResult::Error(e.into()),
        }
    }
}

impl<E: Error + Clone> FieldResult<E> {
    /// Convert the `FieldResult` into a nested `Result<Option<FieldValue>, GdalError> for
    /// convenient error/null handling.
    pub fn into_opt_res(self) -> Result<Option<FieldValue>, E> {
        match self {
            FieldResult::Some(field) => Ok(Some(field)),
            FieldResult::Null => Ok(None),
            FieldResult::Error(e) => Err(e),
        }
    }
}

// For use within the `FieldResult` impl.
// Consider replacing paste as it is unmaintained, but it seems solid enough.
macro_rules! try_into {
    ($name:ident, $type:ty, $fval:ident) => {
        paste! {
            /// Attempt to convert a `FieldResult` into a desired type.
            pub fn [<try_into_ $name>](&self) -> Result<$type, GdalTraitError> {
                let FieldValue::$fval(rv) = self.to_owned().into_res()? else {
                    return Err(
                        GdalTraitError::InvalidFieldValue(
                            format!("Failed to convert {self:?} into a {}", stringify!($ty))
                        )
                    );
                };
                Ok(rv)
            }

            // Holy branching, batman!
            /// Attempt to convert a `FieldResult` into an Option<...> of a desired type.
            pub fn [<try_into_ $name _opt>](&self) -> Result<Option<$type>, GdalTraitError> {
                Ok(match self.to_owned().into_opt_res()? {
                    Some(val) => {
                        let FieldValue::$fval(val) = val else {
                            return Err(
                                GdalTraitError::InvalidFieldValue(
                                    format!("Failed to convert {self:?} into an Option<{}>", stringify!($ty))
                                )
                            );
                        };
                        Some(val)
                    }
                    None => None,
                })
            }
        }
    };
}

impl FieldResult<GdalTraitError> {
    /// Convert the `FieldResult` into a `Result<FieldValue, GdalError>` for convenient
    /// error/null handling.
    ///
    /// `FieldResult::Null` will be treated as GdalTraitError::NullField.
    pub fn into_res(self) -> Result<FieldValue, GdalTraitError> {
        match self {
            FieldResult::Some(field) => Ok(field),
            FieldResult::Null => Err(GdalTraitError::NullField),
            FieldResult::Error(e) => Err(e),
        }
    }

    try_into!(int, i32, IntegerValue);
    try_into!(int_list, Vec<i32>, IntegerListValue);
    try_into!(int64, i64, Integer64Value);
    try_into!(int64_list, Vec<i64>, Integer64ListValue);
    try_into!(string, String, StringValue);
    try_into!(string_list, Vec<String>, StringListValue);
    try_into!(real, f64, RealValue);
    try_into!(real_list, Vec<f64>, RealListValue);
    try_into!(date, NaiveDate, DateValue);
    try_into!(date_time, DateTime<FixedOffset>, DateTimeValue);
}

pub trait FeatureTrait<const N: usize, E>
where
    Self: Sized,
    E: Error + From<GdalTraitError>,
{
    const NUM_FIELDS: usize = N;

    /// Desired fields from the layer.
    const FIELDS: [&'static str; N];

    /// 'Read' fields, geometry, etc. from the source Feature.
    ///
    /// Called by from_feature and from_layer.
    fn read(
        fid: Option<u64>,
        fields: [FieldResult<GdalTraitError>; N],
        geometry: Option<&Geometry>,
    ) -> Result<Self, E>;

    fn from_feature(feature: Feature) -> Result<Self, E> {
        let fields: [FieldResult<GdalTraitError>; N] = Self::FIELDS
            .into_iter()
            .map(|fname| feature.field_index(fname))
            .map(|index| match index {
                Ok(index) => feature.field(index).into(),
                Err(e) => FieldResult::Error(e.into()),
            })
            .collect::<Vec<FieldResult<_>>>()
            .try_into()
            .unwrap();

        Ok(Self::read(feature.fid(), fields, feature.geometry())?)
    }

    fn from_layer(layer: &mut Layer) -> Result<Vec<Self>, E> {
        let field_ids: Vec<Result<usize, GdalError>> = Self::FIELDS
            .into_iter()
            .map(|fname| layer.defn().field_index(fname))
            .collect();

        layer
            .features()
            .into_iter()
            .map(|feature| {
                let fields: [FieldResult<GdalTraitError>; N] = field_ids
                    .iter()
                    .map(|index| match index {
                        Ok(index) => feature.field(*index).into(),
                        Err(e) => FieldResult::Error((e.clone()).into()),
                    })
                    .collect::<Vec<FieldResult<_>>>()
                    .try_into()
                    .unwrap();

                Self::read(feature.fid(), fields, feature.geometry())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use gdal::vector::{Geometry, LayerAccess};
    use gdal::Dataset;
    use thiserror::Error;

    use super::*;

    #[derive(Debug, Error)]
    enum TestError {
        #[error("GDAL Trait Error: {0:?}")]
        GdalTraitError(#[from] GdalTraitError),

        #[error("GDAL Error: {0:?}")]
        GdalError(#[from] GdalError),

        #[error("No Geomety")]
        NoGeometry,

        #[error("geo_type Error: {0}")]
        GeoError(#[from] geo_types::Error),
    }

    #[derive(Debug, PartialEq)]
    struct Country {
        name: String,
        iso_a2: Option<String>,
        iso_a3: String,

        pop_est: f64,
        pop_year: i32,

        geom: geo_types::Geometry<f64>,
    }

    impl FeatureTrait<5, TestError> for Country {
        const FIELDS: [&'static str; Self::NUM_FIELDS] =
            ["NAME", "ISO_A2", "ISO_A3", "POP_EST", "POP_YEAR"];

        fn read(
            _fid: Option<u64>,
            fields: [FieldResult<GdalTraitError>; Self::NUM_FIELDS],
            geometry: Option<&Geometry>,
        ) -> Result<Self, TestError> {
            let [name_field, a2_field, a3_field, pop_est_field, pop_year_field] = fields;

            let name = name_field.try_into_string()?;
            let iso_a2 = a2_field.try_into_string_opt()?;
            let iso_a3 = a3_field.try_into_string()?;

            let pop_est = pop_est_field.try_into_real()?;
            let pop_year = pop_year_field.try_into_int()?;

            let geom = geometry.ok_or(TestError::NoGeometry)?.to_geo()?;

            Ok(Self {
                name,
                iso_a2,
                iso_a3,
                pop_est,
                pop_year,
                geom,
            })
        }
    }

    #[test]
    fn test_from_feature() {
        let ds = Dataset::open("fixtures/ne_110m_admin_0_countries/ne_110m_admin_0_countries.shp")
            .unwrap();

        let layer = ds.layer(0).unwrap();
        let feature = layer.feature(110).unwrap();
        let geom: geo_types::Geometry = feature.geometry().unwrap().to_geo().unwrap();

        let country = Country::from_feature(feature).unwrap();
        assert_eq!(
            country,
            Country {
                name: "Sweden".to_string(),
                iso_a2: Some("SE".to_string()),
                iso_a3: "SWE".to_string(),
                pop_est: 10285453.0,
                pop_year: 2019,
                geom: geom
            }
        );
    }

    #[test]
    fn test_from_layer() {
        let ds = Dataset::open("fixtures/ne_110m_admin_0_countries/ne_110m_admin_0_countries.shp")
            .unwrap();

        let mut layer = ds.layer(0).unwrap();
        let countries = Country::from_layer(&mut layer).unwrap();

        let feature = layer.feature(110).unwrap();
        let geom: geo_types::Geometry = feature.geometry().unwrap().to_geo().unwrap();

        assert_eq!(
            countries.get(110),
            Some(&Country {
                name: "Sweden".to_string(),
                iso_a2: Some("SE".to_string()),
                iso_a3: "SWE".to_string(),
                pop_est: 10285453.0,
                pop_year: 2019,
                geom: geom
            })
        );

        let feature = layer.feature(142).unwrap();
        let geom: geo_types::Geometry = feature.geometry().unwrap().to_geo().unwrap();

        assert_eq!(
            countries.get(142),
            Some(&Country {
                name: "Denmark".to_string(),
                iso_a2: Some("DK".to_string()),
                iso_a3: "DNK".to_string(),
                pop_est: 5818553.0,
                pop_year: 2019,
                geom: geom
            })
        );
    }
}
