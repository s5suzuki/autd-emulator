#[macro_use]
extern crate conrod_core;

mod autd_event_handler;
mod camera_helper;
mod color;
mod interface;
mod parser;
mod settings;
mod ui;
mod viewer_controller;

use std::sync::mpsc;

use crate::{
    autd_event_handler::AUTDEventHandler, settings::Setting, ui::UiView,
    viewer_controller::ViewController,
};
use acoustic_field_viewer::{
    coloring_method::coloring_hsv,
    view::{
        AcousticFiledSliceViewer, SoundSourceViewer, UpdateHandler, ViewWindow, ViewerSettings,
    },
};
use piston_window::{Button, Window};

type Vector3 = vecmath::Vector3<f32>;
type Matrix4 = vecmath::Matrix4<f32>;

fn main() {
    let mut setting = Setting::load("setting.json");

    let mut interf = interface::Interface::open(&format!("127.0.0.1:{}", setting.port)).unwrap();
    let (tx_autd_event, rx_autd_event) = mpsc::channel();
    interf.start(tx_autd_event).unwrap();

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

    let source_viewer = SoundSourceViewer::new();
    let acoustic_field_viewer = AcousticFiledSliceViewer::new(setting.slice_model);

    let (from_ui, to_cnt) = mpsc::channel();
    let (from_cnt, to_ui) = mpsc::channel();

    let (mut field_view, mut field_window) = ViewWindow::new(
        vec![],
        source_viewer,
        acoustic_field_viewer,
        settings,
        [setting.window_width, setting.window_height],
    );

    let mut autd_event_handler = AUTDEventHandler::new(rx_autd_event);
    let mut viewer_controller = ViewController::new(to_cnt, from_cnt);
    let update = |update_handler: &mut UpdateHandler, button: Option<Button>| {
        autd_event_handler.update(update_handler);
        viewer_controller.update(update_handler, button);
    };
    field_view.update = Some(update);

    let (mut ui_view, mut ui_window) = UiView::new(to_ui, from_ui);
    while let (Some(e_field), Some(e_ui)) = (field_window.next(), ui_window.next()) {
        field_view.renderer(&mut field_window, e_field);
        ui_view.renderer(&mut ui_window, e_ui);
    }

    interf.close();

    setting.slice_model = field_view.get_slice_model();

    let current_size = field_window.draw_size();
    setting.window_width = current_size.width as u32;
    setting.window_height = current_size.height as u32;
    setting.save("setting.json");
}
