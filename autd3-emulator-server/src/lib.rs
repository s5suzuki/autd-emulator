mod autd_data;
mod interface;
mod parser;
mod server;

type Vector3 = vecmath::Vector3<f32>;

pub use autd_data::*;
pub use server::AUTDServer;
