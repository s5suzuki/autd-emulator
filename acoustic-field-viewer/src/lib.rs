#[macro_use]
extern crate bitflags;

pub mod camera_helper;
pub mod common;
pub mod dir_viewer;
pub mod field_compute_pipeline;
pub mod renderer;
pub mod slice_viewer;
pub mod sound_sources;
pub mod trans_viewer;
mod update_flag;
mod viewer_settings;

pub use update_flag::UpdateFlag;
pub use viewer_settings::ViewerSettings;

pub type Vector2 = vecmath::Vector2<f32>;
pub type Vector3 = vecmath::Vector3<f32>;
pub type Vector4 = vecmath::Vector4<f32>;
pub type Matrix3 = vecmath::Matrix3<f32>;
pub type Matrix4 = vecmath::Matrix4<f32>;
