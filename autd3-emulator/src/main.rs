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

use std::f32::consts::PI;
use std::sync::mpsc;

use crate::autd_event_handler::AUTDEventHandler;
use crate::settings::Setting;
use crate::ui::UiView;
use crate::viewer_controller::ViewController;
use acoustic_field_viewer::coloring_method::coloring_hsv;
use acoustic_field_viewer::sound_source::SoundSource;
use acoustic_field_viewer::view::event::*;
use acoustic_field_viewer::view::{
    AcousticFiledSliceViewer, SoundSourceViewer, UpdateHandler, ViewWindow, ViewerSettings,
};
use autd3_core::hardware_defined::{NUM_TRANS_X, NUM_TRANS_Y};

type Vector3 = vecmath::Vector3<f32>;
type Matrix4 = vecmath::Matrix4<f32>;

fn main() {
    let setting = Setting::load("setting.json");
    let mut interf = interface::Interface::open(&format!("127.0.0.1:{}", setting.port)).unwrap();

    let (tx_autd_event, rx_autd_event) = mpsc::channel();
    interf.start(tx_autd_event).unwrap();

    let trans_mm = autd3_core::hardware_defined::TRANS_SPACING_MM as f32;
    let mut settings = ViewerSettings::new(
        40e3,
        trans_mm,
        coloring_hsv,
        scarlet::colormap::ListedColorMap::inferno(),
    );
    settings.color_scale = 0.6;
    settings.slice_alpha = 0.95;

    let source_viewer = SoundSourceViewer::new();
    let mut acoustic_field_viewer = AcousticFiledSliceViewer::new();
    acoustic_field_viewer.translate([trans_mm * 8.5, trans_mm * 6.5, 150.]);

    let (from_ui, to_cnt) = mpsc::channel();
    let (from_cnt, to_ui) = mpsc::channel();

    let (mut field_view, mut field_window) =
        ViewWindow::new(vec![], source_viewer, acoustic_field_viewer, settings);

    let autd_event_handler = AUTDEventHandler::new(rx_autd_event);
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
    setting.save("setting.json");
}
