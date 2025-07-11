use crate::args::guess_details;
use crate::ffts::{take_fft, FftConfig, Windowing};
use crate::samples::SampleFile;
use crate::{u64_from, usize_from, Samples};
use anyhow::{anyhow, Result};
use egui::{ColorImage, Vec2};
use num_traits::Zero;
use poll_promise::Promise;
use rustfft::num_complex::Complex;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

pub fn display(filename: &Option<PathBuf>) -> Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    let filename = filename
        .as_ref()
        .ok_or_else(|| anyhow!("filename currently required"))?;

    let filename = filename
        .to_str()
        .ok_or_else(|| anyhow!("filename contains invalid UTF-8"))?
        .to_string();

    let details = guess_details(&filename, None, None)?;
    let file = SampleFile::new(
        fs::File::open(filename)?,
        details.format,
        details.sample_rate,
    );

    eframe::run_native(
        "eframe template",
        native_options,
        Box::new(|cc| Ok(Box::new(ManageApp::new(cc, Arc::new(file))))),
    )
    .expect("TODO: non-anyhow-compatible error");
    Ok(())
}

struct ManageApp {
    start: f32,
    end: f32,

    fft_width: f32,

    samples: Arc<dyn Samples>,

    texture: Option<egui::TextureHandle>,
    next_image: Option<Arc<ColorImage>>,
    renderation: Option<Promise<ColorImage>>,
}

impl ManageApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, samples: Arc<dyn Samples>) -> Self {
        let mut us = ManageApp {
            samples,
            start: 46.0,
            end: 46.3,
            fft_width: 512.,
            texture: None,
            next_image: None,
            renderation: None,
        };

        us.trigger_redraw();

        us
    }

    fn trigger_redraw(&mut self) {
        let samples = Arc::clone(&self.samples);
        let fft_width = self.fft_width as usize;

        let fft = rustfft::FftPlanner::<f32>::new().plan_fft_forward(fft_width);

        let start = self.start;
        let end = self.end;

        self.renderation = Some(Promise::spawn_thread("renderation", move || {
            let hoight = 2048;
            let mut buf = vec![egui::Color32::TRANSPARENT; fft_width * hoight];

            let start_sample = (samples.len() as f32 * start / 100.) as u64;
            let end_sample = (samples.len() as f32 * end / 100.) as u64;
            let fft = take_fft(
                &*samples,
                Some((start_sample, end_sample)),
                &FftConfig {
                    width: fft_width,
                    windowing: Windowing::BlackmanHarris,
                },
                hoight,
            )
            .expect("Failed to take FFT");
            for row in 0..fft.output_len() {
                for (i, c) in fft.get(row).iter().enumerate() {
                    let b = (c / 10. * 256.) as u8;
                    buf[row * fft_width + i] = egui::Color32::from_rgb(0, 0, b);
                }
            }

            ColorImage {
                size: [fft_width, hoight],
                pixels: buf,
            }
        }));
    }
}

impl eframe::App for ManageApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(16.0);

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.renderation.as_ref().and_then(|v| v.ready()).is_some() {
                match self.renderation.take().expect("just checked").try_take() {
                    Ok(image) => {
                        self.next_image = Some(Arc::new(image));
                    }
                    Err(_) => unreachable!(),
                }
            }

            if let Some(next_image) = self.next_image.take() {
                self.texture = Some(ui.ctx().load_texture(
                    "next_image",
                    next_image,
                    Default::default(),
                ));
            }

            ui.spacing_mut().slider_width = ui.available_width() - 100.;

            let sliders = vec![
                ui.add(egui::Slider::new(&mut self.start, 0.0..=100.0).text("start")),
                ui.add(egui::Slider::new(&mut self.end, 0.0..=100.0).text("end")),
                ui.add(egui::Slider::new(&mut self.fft_width, 4.0..=4096.0).text("fft")),
            ];

            if sliders.iter().any(|slider| slider.changed()) {
                self.trigger_redraw();
            }

            ui.separator();

            // println!("{:?}", ui.available_size());
            if let Some(texture) = self.texture.as_ref() {
                ui.image((
                    texture.id(),
                    Vec2::new(
                        ui.available_size_before_wrap().x,
                        ui.available_height() - 60.0,
                    ),
                ));
            } else {
                ui.spinner();
            }

            ui.separator();

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                egui::warn_if_debug_build(ui);
            });
        });
    }
}
