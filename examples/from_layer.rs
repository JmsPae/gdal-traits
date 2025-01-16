// Simple example which only uses the FromFeature trait.

use gdal::errors::GdalError;
use gdal::vector::{FieldValue, Geometry};
use gdal::Dataset;
use gdal_traits::*;
use thiserror::Error;

// Using thiserror for convenience.
#[derive(Debug, Error)]
enum CountryError {
    #[error("GDAL Trait Error: {0:?}")]
    GdalTraitError(#[from] GdalTraitError),

    // GdalTraitError already implements From<GdalError> and it isn't required for the trait impl.
    // However, it can still be convenient when e.g. working with geometry.
    #[error("GDAL Error: {0:?}")]
    GdalError(#[from] GdalError),

    #[error("No Geomety")]
    NoGeometry,

    #[error("No FID")]
    NoFID,

    #[error("geo_type Error: {0}")]
    GeoError(#[from] geo_types::Error),
}

#[allow(dead_code)]
#[derive(Debug)]
struct Country {
    id: u64,
    name: String,
    iso_a2: String,
    iso_a3: String,

    pop_est: Option<f64>,
    pop_year: Option<i32>,

    geom: geo_types::Geometry<f64>,
}

// The FromFeature trait requires a const usize for the number of fields, and an Error type.
impl FromFeature<5, CountryError> for Country {
    const FIELDS: [&'static str; Self::NUM_FIELDS] =
        ["NAME", "ISO_A2_EH", "ISO_A3_EH", "POP_EST", "POP_YEAR"];

    // NUM_FIELDS is derived from the trait usize const.
    fn read(
        fid: Option<u64>,
        fields: [FieldResult<GdalTraitError>; Self::NUM_FIELDS],
        geometry: Option<&Geometry>,
    ) -> Result<Self, CountryError> {
        // The index maps directly to that of FIELDS, regardless of cases of NULL or an error
        // occurs e.g. if the requested field does not exist. It's up to you how to handle those
        // cases.
        let [name_field, a2_field, a3_field, pop_est_field, pop_year_field] = fields;

        // into_res() treats NULL as an error.
        let FieldValue::StringValue(name) = name_field.into_res()? else {
            return Err(CountryError::NoGeometry);
        };

        // Convenience functions, treats NULL as Error.
        let iso_a2 = a2_field.try_into_string()?;
        let iso_a3 = a3_field.try_into_string()?;

        // treats NULL as None.
        let pop_est = pop_est_field.try_into_real_opt()?;
        let pop_year = pop_year_field.try_into_int_opt()?;

        let geom = geometry.ok_or(CountryError::NoGeometry)?.to_geo()?;

        Ok(Self {
            id: fid.ok_or(CountryError::NoFID)?,
            name,
            iso_a2,
            iso_a3,
            pop_est,
            pop_year,
            geom,
        })
    }
}

fn main() {
    let ds =
        Dataset::open("fixtures/ne_110m_admin_0_countries/ne_110m_admin_0_countries.shp").unwrap();

    // Dataset only has one layer.
    let mut layer = ds.layer(0).unwrap();
    let countries = Country::from_layer(&mut layer).unwrap();

    println!("First 10 countries:");
    for country in countries.iter().take(10) {
        println!(
            "{}: NAME {} ISO_A2 {} ISO_A3 {} POP_EST {:?} POP_YEAR {:?}",
            country.id,
            country.name,
            country.iso_a2,
            country.iso_a3,
            country.pop_est,
            country.pop_year
        );
    }
}
