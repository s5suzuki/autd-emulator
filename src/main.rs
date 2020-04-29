mod interface;
mod parser;

use std::sync::mpsc;

use acoustic_field_viewer::coloring_method::coloring_hsv;
use acoustic_field_viewer::sound_source::SoundSource;
use acoustic_field_viewer::view::event::*;
use acoustic_field_viewer::view::{
    AcousticFiledSliceViewer, SoundSourceViewer, UpdateHandler, ViewWindow, ViewerSettings,
};

use parser::{AUTDData, Geometry};

const NUM_TRANS_X: usize = 18;
const NUM_TRANS_Y: usize = 14;
const TRANS_SIZE: f32 = 10.18;

use std::f32::consts::PI;

fn is_missing_transducer(x: usize, y: usize) -> bool {
    y == 1 && (x == 1 || x == 2 || x == 16)
}

fn make_autd_transducers(geo: Geometry) -> Vec<SoundSource> {
    let mut transducers = Vec::new();
    for y in 0..NUM_TRANS_Y {
        for x in 0..NUM_TRANS_X {
            if is_missing_transducer(x, y) {
                continue;
            }
            let x_dir = vecmath::vec3_scale(geo.right, TRANS_SIZE * x as f32);
            let y_dir = vecmath::vec3_scale(geo.up, TRANS_SIZE * y as f32);
            let zdir = vecmath::vec3_cross(geo.right, geo.up);
            let pos = geo.origin;
            let pos = vecmath::vec3_add(pos, x_dir);
            let pos = vecmath::vec3_add(pos, y_dir);
            transducers.push(SoundSource::new(pos, zdir, PI));
        }
    }
    transducers
}

fn main() {
    let mut interf = interface::Interface::open("127.0.0.1:50632").unwrap();

    let (tx, rx) = mpsc::channel();
    interf.start(tx).unwrap();

    let mut settings = ViewerSettings::new(
        40e3,
        TRANS_SIZE,
        coloring_hsv,
        scarlet::colormap::ListedColorMap::inferno(),
    );
    settings.color_scale = 0.6;
    settings.slice_alpha = 0.95;
    let source_viewer = SoundSourceViewer::new();
    let mut acoustic_field_viewer = AcousticFiledSliceViewer::new();
    acoustic_field_viewer.translate([TRANS_SIZE * 8.5, TRANS_SIZE * 6.5, 150.]);

    let mut window = ViewWindow::new(vec![], source_viewer, acoustic_field_viewer, settings);

    let update = |update_handler: &mut UpdateHandler, _button: Option<Button>| {
        if let Ok(d) = rx.try_recv() {
            let data = parser::parse(d);
            for d in data {
                match d {
                    AUTDData::Geometries(geos) => {
                        for geo in geos {
                            let transducers = make_autd_transducers(geo);
                            for trans in transducers {
                                update_handler.sources.borrow_mut().push(trans);
                            }
                        }
                        update_handler.update_source_pos();
                        update_handler.update_source_phase();
                    }
                    AUTDData::Gain(gain) => {
                        for (&phase, source) in gain
                            .phases
                            .iter()
                            .zip(update_handler.sources.borrow_mut().iter_mut())
                        {
                            source.phase = 2.0 * PI * (1.0 - (phase as f32 / 255.0));
                        }
                        update_handler.update_source_phase();
                    }
                    AUTDData::Clear => {
                        update_handler.sources.borrow_mut().clear();
                        update_handler.update_source_pos();
                        update_handler.update_source_phase();
                    }
                    _ => (),
                }
            }
        }
    };

    window.update = Some(update);
    window.start();
    interf.close();
}
