/*
 * File: texture.rs
 * Project: common
 * Created Date: 08/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 08/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::path::Path;

use gfx::{format::Srgba8, handle::ShaderResourceView};
use image::DynamicImage;

pub fn create_texture_resource<P: AsRef<Path>, F: gfx::Factory<R>, R: gfx::Resources>(
    path: P,
    factory: &mut F,
) -> Result<ShaderResourceView<R, [f32; 4]>, anyhow::Error> {
    let img = image::open(path)?;

    let img = match img {
        DynamicImage::ImageRgba8(img) => img,
        img => img.to_rgba8(),
    };

    let (width, height) = img.dimensions();

    fn create_texture<T, F, R>(
        factory: &mut F,
        kind: gfx::texture::Kind,
        data: &[&[u8]],
    ) -> Result<
        (
            gfx::handle::Texture<R, T::Surface>,
            gfx::handle::ShaderResourceView<R, T::View>,
        ),
        anyhow::Error,
    >
    where
        F: gfx::Factory<R>,
        R: gfx::Resources,
        T: gfx::format::TextureFormat,
    {
        use gfx::memory::{Bind, Usage};
        use gfx::{format, texture};
        use gfx_core::memory::Typed;
        use gfx_core::texture::Mipmap;

        let surface = <T::Surface as format::SurfaceTyped>::get_surface_type();
        let num_slices = kind.get_num_slices().unwrap_or(1) as usize;
        let num_faces = if kind.is_cube() { 6 } else { 1 };
        let desc = texture::Info {
            kind: kind,
            levels: (data.len() / (num_slices * num_faces)) as texture::Level,
            format: surface,
            bind: Bind::SHADER_RESOURCE,
            usage: Usage::Dynamic,
        };
        let cty = <T::Channel as format::ChannelTyped>::get_channel_type();
        let raw = factory.create_texture_raw(desc, Some(cty), Some((data, Mipmap::Provided)))?;
        let levels = (0, raw.get_info().levels - 1);
        let tex = Typed::new(raw);
        let view =
            factory.view_texture_as_shader_resource::<T>(&tex, levels, format::Swizzle::new())?;
        Ok((tex, view))
    }

    let (width, height) = (width as u16, height as u16);
    let tex_kind = gfx::texture::Kind::D2(width, height, gfx::texture::AaMode::Single);
    let filter_method = gfx::texture::FilterMethod::Scale;
    let wrap_mode_u = gfx::texture::WrapMode::Tile;
    let wrap_mode_v = gfx::texture::WrapMode::Tile;
    let mut sampler_info = gfx::texture::SamplerInfo::new(filter_method, wrap_mode_u);
    sampler_info.wrap_mode.1 = wrap_mode_v;
    sampler_info.border = [0.0, 0.0, 0.0, 1.0].into();

    let (_, view) = create_texture::<Srgba8, F, R>(factory, tex_kind, &[&img])?;

    Ok(view)
}
