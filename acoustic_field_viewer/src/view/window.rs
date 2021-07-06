/*
 * File: windows.rs
 * Project: view
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 06/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use std::cell::RefCell;
use std::rc::Rc;

use camera_controllers::{Camera, CameraPerspective, FirstPerson, FirstPersonSettings};
use piston_window::Window;
use piston_window::*;

use crate::sound_source::SoundSource;
use crate::view::{AcousticFiledSliceViewer, SoundSourceViewer, ViewerSettings};
use crate::Matrix4;

pub struct UpdateHandler {
    update_source_pos: bool,
    update_source_drive: bool,
    update_camera_pos: bool,
    pub sound_source_viewer: SoundSourceViewer,
    pub field_slice_viewer: AcousticFiledSliceViewer,
    pub sources: Rc<RefCell<Vec<SoundSource>>>,
    pub settings: Rc<RefCell<ViewerSettings>>,
    pub camera: Camera<f32>,
}

impl UpdateHandler {
    fn new(
        sources: Rc<RefCell<Vec<SoundSource>>>,
        sound_source_viewer: SoundSourceViewer,
        field_slice_viewer: AcousticFiledSliceViewer,
        settings: Rc<RefCell<ViewerSettings>>,
        camera: Camera<f32>,
    ) -> UpdateHandler {
        UpdateHandler {
            update_source_drive: false,
            update_source_pos: false,
            update_camera_pos: false,
            sound_source_viewer,
            field_slice_viewer,
            sources,
            settings,
            camera,
        }
    }

    fn update_sources(&mut self) {
        if self.update_source_drive {
            self.sound_source_viewer.update_drive();
            self.field_slice_viewer.update_source_drive();
            self.update_source_drive = false;
        }
        if self.update_source_pos {
            self.sound_source_viewer.update_position();
            self.field_slice_viewer.update_source_pos();
            self.update_source_pos = false;
        }
        if self.update_camera_pos {
            self.sound_source_viewer.camera_pos_update();
            self.update_camera_pos = false;
        }
    }

    pub fn update_camera_pos(&mut self) {
        self.update_camera_pos = true;
    }

    pub fn update_drive(&mut self) {
        self.update_source_drive = true;
    }

    pub fn update_position(&mut self) {
        self.update_source_pos = true;
    }

    pub fn drive_updated(&self) -> bool {
        self.update_source_drive
    }

    pub fn position_updated(&self) -> bool {
        self.update_source_pos
    }
}

pub struct ViewWindow<F>
where
    F: FnMut(&mut UpdateHandler, Option<Button>),
{
    pub update: Option<F>,
    update_handler: UpdateHandler,
    projection: Matrix4,
}

impl<F> ViewWindow<F>
where
    F: FnMut(&mut UpdateHandler, Option<Button>),
{
    pub fn new(
        sources: Vec<SoundSource>,
        sound_source_viewer: SoundSourceViewer,
        field_slice_viewer: AcousticFiledSliceViewer,
        settings: ViewerSettings,
    ) -> (ViewWindow<F>, PistonWindow) {
        let opengl = OpenGL::V4_5;
        let mut window: PistonWindow = WindowSettings::new("", [640, 480])
            .exit_on_esc(true)
            .samples(4)
            .graphics_api(opengl)
            .build()
            .unwrap();
        window.set_ups(60);
        window.set_max_fps(1000);
        let projection = ViewWindow::<F>::get_projection(&window);
        let first_person =
            FirstPerson::new([90., -250.0, 120.0], FirstPersonSettings::keyboard_wasd());
        let mut camera = first_person.camera(0.);
        camera.set_yaw_pitch(0., -std::f32::consts::PI / 2.0);

        let mut sound_source_viewer = sound_source_viewer;
        let mut field_slice_viewer = field_slice_viewer;

        let ref_sources = Rc::new(RefCell::new(sources));
        sound_source_viewer.sources = Rc::downgrade(&ref_sources);
        field_slice_viewer.sources = Rc::downgrade(&ref_sources);

        let ref_settings = Rc::new(RefCell::new(settings));
        sound_source_viewer.settings = Rc::downgrade(&ref_settings);
        field_slice_viewer.settings = Rc::downgrade(&ref_settings);

        sound_source_viewer.init_model();
        field_slice_viewer.render_setting(&window, opengl);
        sound_source_viewer.render_setting(&mut window, opengl);

        (
            ViewWindow {
                update: None,
                update_handler: UpdateHandler::new(
                    ref_sources,
                    sound_source_viewer,
                    field_slice_viewer,
                    ref_settings,
                    camera,
                ),
                projection,
            },
            window,
        )
    }

    pub fn renderer(&mut self, window: &mut PistonWindow, event: Event) {
        if let Some(update_fn) = &mut self.update {
            update_fn(&mut self.update_handler, event.press_args());
            self.update_handler.update_sources();
        }

        let cam_orth = self.update_handler.camera.orthogonal();
        let projection = self.projection;
        window.draw_3d(&event, |window| {
            window
                .encoder
                .clear(&window.output_color, [0.3, 0.3, 0.3, 1.0]);
            window.encoder.clear_depth(&window.output_stencil, 1.0);

            self.update_handler
                .sound_source_viewer
                .renderer(window, &event, cam_orth, projection);
            self.update_handler
                .field_slice_viewer
                .renderer(window, &event, cam_orth, projection);
        });
        if event.resize_args().is_some() {
            self.projection = ViewWindow::<F>::get_projection(&window);
        }
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
}
