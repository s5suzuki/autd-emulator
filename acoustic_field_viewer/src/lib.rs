#[macro_use]
extern crate gfx;
#[macro_use]
extern crate bitflags;

pub mod axis_3d;
pub mod camera_helper;
mod common;
pub mod sound_source;
pub mod view;

pub type Vector3 = vecmath::Vector3<f32>;
pub type Vector4 = vecmath::Vector4<f32>;
pub type Matrix3 = vecmath::Matrix3<f32>;
pub type Matrix4 = vecmath::Matrix4<f32>;
