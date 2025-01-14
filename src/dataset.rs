use crate::FeatureTrait;

/// GDAL Layers can be specified by either their index, or their name.
pub enum DatasetLayer {
    Name(String),
    Index(usize),
}

pub trait DatasetTrait {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_dataset() {}
}
