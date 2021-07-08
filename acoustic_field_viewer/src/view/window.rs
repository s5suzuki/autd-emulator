/*
 * File: windows.rs
 * Project: view
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 08/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use camera_controllers::{Camera, CameraPerspective, FirstPerson, FirstPersonSettings};
use piston_window::*;

use crate::{
    sound_source::SoundSource,
    view::{AcousticFiledSliceViewer, SoundSourceViewer, UpdateFlag, ViewerSettings},
    Matrix4,
};

pub struct ViewWindow {
    projection: Matrix4,
    pub sound_source_viewer: SoundSourceViewer,
    pub field_slice_viewer: AcousticFiledSliceViewer,
    pub camera: Camera<f32>,
}

impl ViewWindow {
    pub fn new<S: Into<Size>>(
        model: Matrix4,
        settings: &ViewerSettings,
        size: S,
    ) -> (ViewWindow, PistonWindow) {
        let opengl = OpenGL::V4_5;
        let mut window: PistonWindow = WindowSettings::new("", size)
            .exit_on_esc(true)
            .samples(4)
            .graphics_api(opengl)
            .vsync(true)
            .build()
            .unwrap();
        window.set_ups(60);
        window.set_max_fps(1000);

        let projection = ViewWindow::get_projection(&window);
        let first_person =
            FirstPerson::new([90., -250.0, 120.0], FirstPersonSettings::keyboard_wasd());
        let mut camera = first_person.camera(0.);
        camera.set_yaw_pitch(0., -std::f32::consts::PI / 2.0);

        let sound_source_viewer = SoundSourceViewer::new(&mut window, opengl);
        let field_slice_viewer = AcousticFiledSliceViewer::new(model, &window, opengl, settings);

        (
            ViewWindow {
                projection,
                sound_source_viewer,
                field_slice_viewer,
                camera,
            },
            window,
        )
    }

    pub fn renderer(
        &mut self,
        window: &mut PistonWindow,
        event: Event,
        settings: &ViewerSettings,
        sources: &[SoundSource],
        update_flag: UpdateFlag,
    ) {
        let cam_orth = self.camera.orthogonal();
        let projection = self.projection;

        self.field_slice_viewer.update(
            window,
            &event,
            (cam_orth, projection),
            settings,
            sources,
            update_flag,
        );
        self.sound_source_viewer.update(
            window,
            &event,
            (cam_orth, projection),
            settings,
            sources,
            update_flag,
        );

        if event.resize_args().is_some() {
            self.projection = ViewWindow::get_projection(&window);
        }

        window.draw_3d(&event, |window| {
            window
                .encoder
                .clear(&window.output_color, [0.3, 0.3, 0.3, 1.0]);
            window.encoder.clear_depth(&window.output_stencil, 1.0);

            self.sound_source_viewer.renderer(window);

            self.field_slice_viewer.renderer(window);
        });
    }

    fn get_projection(w: &PistonWindow) -> Matrix4 {
        let draw_size = w.window.draw_size();
        CameraPerspective {
            fov: 60.0,
            near_clip: 0.1,
            far_clip: 1000.0,
            aspect_ratio: (draw_size.width as f32) / (draw_size.height as f32),
        }
        .projection()
    }

    pub fn get_slice_model(&self) -> Matrix4 {
        self.field_slice_viewer.model()
    }
}
