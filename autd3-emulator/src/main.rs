#[macro_use]
extern crate conrod_core;

mod camera_helper;
mod color;
mod settings;
mod ui;
// mod viewer_controller;

use std::{f32::consts::PI, sync::mpsc};

use crate::{settings::Setting, ui::UiView};
use acoustic_field_viewer::{
    coloring_method::coloring_hsv,
    sound_source::SoundSource,
    view::{UpdateFlag, ViewWindow, ViewerSettings},
};
use autd3_core::hardware_defined::{NUM_TRANS_X, NUM_TRANS_Y, TRANS_SPACING_MM};
use autd3_emulator_server::{AUTDData, AUTDServer, Geometry};
use piston_window::Window;

type Vector3 = vecmath::Vector3<f32>;
type Matrix4 = vecmath::Matrix4<f32>;

fn make_autd_transducers(geo: Geometry) -> Vec<SoundSource> {
    let mut transducers = Vec::new();
    for y in 0..NUM_TRANS_Y {
        for x in 0..NUM_TRANS_X {
            if autd3_core::hardware_defined::is_missing_transducer(x, y) {
                continue;
            }
            let x_dir = vecmath::vec3_scale(geo.right, TRANS_SPACING_MM as f32 * x as f32);
            let y_dir = vecmath::vec3_scale(geo.up, TRANS_SPACING_MM as f32 * y as f32);
            let zdir = vecmath::vec3_cross(geo.right, geo.up);
            let pos = geo.origin;
            let pos = vecmath::vec3_add(pos, x_dir);
            let pos = vecmath::vec3_add(pos, y_dir);
            transducers.push(SoundSource::new(pos, zdir, 0.0, 0.0));
        }
    }
    transducers
}

fn main() {
    let mut setting = Setting::load("setting.json");

    let mut autd_server = AUTDServer::new(&format!("127.0.0.1:{}", setting.port)).unwrap();

    let trans_mm = autd3_core::hardware_defined::TRANS_SPACING_MM as f32;
    let mut settings = ViewerSettings::new(
        40e3,
        setting.wave_length,
        trans_mm,
        coloring_hsv,
        scarlet::colormap::ListedColorMap::inferno(),
        (setting.slice_width, setting.slice_height),
    );
    settings.color_scale = 0.6;
    settings.slice_alpha = 0.95;

    let (from_ui, to_cnt) = mpsc::channel();
    let (from_cnt, to_ui) = mpsc::channel();

    let (mut field_view, mut field_window) =
        ViewWindow::new(&settings, [setting.window_width, setting.window_height]);

    // let mut autd_event_handler = AUTDEventHandler::new(rx_autd_event);
    // let mut viewer_controller = ViewController::new(to_cnt, from_cnt);
    // let update = |update_handler: &mut UpdateHandler, button: Option<Button>| {
    // autd_event_handler.update(update_handler);
    // viewer_controller.update(update_handler, button);
    // };
    // field_view.update = Some(update);

    let mut sources = Vec::new();
    let mut last_amp = Vec::new();

    let (mut ui_view, mut ui_window) = UiView::new(to_ui, from_ui);
    while let (Some(e_field), Some(e_ui)) = (field_window.next(), ui_window.next()) {
        let mut update_flag = UpdateFlag::empty();

        autd_server.update(|data| {
            for d in data {
                match d {
                    AUTDData::Geometries(geometries) => {
                        sources.clear();
                        for geometry in geometries {
                            let transducers = make_autd_transducers(geometry);
                            for trans in transducers {
                                sources.push(trans);
                            }
                        }
                        update_flag |= UpdateFlag::UPDATE_SOURCE_POS;
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AUTDData::Gain(gain) => {
                        for ((&phase, &amp), source) in gain
                            .phases
                            .iter()
                            .zip(gain.amps.iter())
                            .zip(sources.iter_mut())
                        {
                            source.amp = (amp as f32 / 510.0 * std::f32::consts::PI).sin();
                            source.phase = 2.0 * PI * (1.0 - (phase as f32 / 255.0));
                        }
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AUTDData::Clear => {
                        for source in sources.iter_mut() {
                            source.amp = 0.;
                            source.phase = 0.;
                        }
                    }
                    AUTDData::Pause => {
                        last_amp.clear();
                        for source in sources.iter_mut() {
                            last_amp.push(source.amp);
                            source.amp = 0.;
                        }
                    }
                    AUTDData::Resume => {
                        for (source, &amp) in sources.iter_mut().zip(last_amp.iter()) {
                            source.amp = amp;
                        }
                        last_amp.clear();
                    }
                    _ => (),
                }
            }
        });

        field_view.renderer(&mut field_window, e_field, &settings, &sources, update_flag);
        ui_view.renderer(&mut ui_window, e_ui);
    }

    autd_server.close();

    setting.slice_model = field_view.get_slice_model();

    let current_size = field_window.draw_size();
    setting.window_width = current_size.width as u32;
    setting.window_height = current_size.height as u32;
    setting.save("setting.json");
}
