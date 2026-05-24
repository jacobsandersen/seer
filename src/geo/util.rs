use geojson::{GeoJson, GeometryValue};

pub struct Coords {
  pub longitude: f64,
  pub latitude: f64
}

/// `extract_coords` attempts to extract the longitude and latitude values from
/// a given GeoJson.
/// 
/// Given a GeoJson::Feature, it will try to extract to Geometry from it and then 
/// recurse with that.
/// 
/// Given a GeoJson::Geometry, it will extract the coordinates if the Geometry is a
/// GeometryValue::Point. Otherwise, it will return None.
/// 
/// Given a GeoJson::FeatureCollection, it will try to find the first Feature in the
/// collection and recurse with it.
pub fn extract_coords(geo: GeoJson) -> Option<Coords> {
  match geo {
    GeoJson::Feature(f) => {
      extract_coords(GeoJson::Geometry(f.geometry?))
    },
    GeoJson::Geometry(g) => {
      match g.value {
        GeometryValue::Point { coordinates } => {
          Some(Coords { longitude: coordinates[0], latitude: coordinates[1] })
        },
        _ => None
      }
    },
    GeoJson::FeatureCollection(fc) => {
      match fc.features.len() {
        0 => None,
        _ => extract_coords(GeoJson::Feature(fc.features[0].clone()))
      }
    }
  }
}