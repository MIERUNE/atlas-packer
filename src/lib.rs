mod disjoint_set;
mod file_reader;
pub mod export;
pub mod pack;
pub mod place;
pub mod texture;

pub use export::ExportError;

pub type ClusterID = String;
pub type AtlasID = usize;
pub type PolygonID = String;
