#[macro_use]
extern crate gfx;

mod common;
pub mod sound_source;
pub mod view;

type Vector3 = vecmath::Vector3<f32>;
type Matrix4 = vecmath::Matrix4<f32>;

pub use common::color;
pub use common::coloring_method;
