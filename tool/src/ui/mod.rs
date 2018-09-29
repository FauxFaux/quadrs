use conrod;
use conrod::backend::glium::glium;
use conrod::backend::glium::glium::Surface;
use conrod::color;
use conrod::glium::texture::ClientFormat;
use conrod::glium::texture::RawImage2d;
use conrod::text;
use conrod::widget;
use conrod::Borderable;
use conrod::Colorable;
use conrod::Labelable;
use conrod::Positionable;
use conrod::Sizeable;
use conrod::Widget;
use failure::Error;
use palette;
use rustfft::algorithm::Radix4;
use rustfft::num_complex::Complex;

use octagon::Samples;

mod support;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Params {
    width: u32,
    height: u32,
    fft_width: usize,
    stride: u32,
    stretch: isize,
}

pub fn display(samples: &mut Samples) -> Result<(), Error> {
    const WIDTH: u32 = 800;
    const HEIGHT: u32 = 600;

    // Build the window.
    let mut events_loop = glium::glutin::EventsLoop::new();
    let window = glium::glutin::WindowBuilder::new()
        .with_title("quadrs")
        .with_dimensions(WIDTH, HEIGHT);
    let context = glium::glutin::ContextBuilder::new()
        .with_vsync(true)
        .with_multisampling(4);
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    // construct our `Ui`.
    let mut ui = conrod::UiBuilder::new([WIDTH as f64, HEIGHT as f64]).build();
    ui.fonts.insert(
        text::FontCollection::from_bytes(
            &include_bytes!("../../../assets/NotoSans-Regular.ttf")[..],
        )?
        .into_font()?,
    );

    // A type used for converting `conrod::render::Primitives` into `Command`s that can be used
    // for drawing to the glium `Surface`.
    let mut renderer = conrod::backend::glium::Renderer::new(&display).unwrap();

    // The `WidgetId` for our background and `Image` widgets.
    widget_ids!(struct Ids {
        root,
        background,
        background_scrollbar,
        canvas,
        buttons,
        fft_up,
        fft_label,
        fft_down,
        stretch_up,
        stretch_label,
        stretch_down,
        stride_up,
        stride_label,
        stride_down,
    });
    let ids = Ids::new(ui.widget_id_generator());

    let mut params = Params {
        width: 0,
        height: 0,
        stride: 1,
        fft_width: 8,
        stretch: 4,
    };

    let mut prev_params = params;

    let mut image_map = conrod::image::Map::<glium::texture::Texture2d>::new();
    let mut canvas_img = None;

    // Poll events from the window.
    let mut event_loop = support::EventLoop::new();
    'main: loop {
        // Handle all events.
        for event in event_loop.next(&mut events_loop) {
            // Use the `winit` backend feature to convert the winit event to a conrod one.
            if let Some(event) = conrod::backend::winit::convert_event(event.clone(), &display) {
                ui.handle_event(event);
            }

            match event {
                glium::glutin::Event::WindowEvent { event, .. } => match event {
                    // Break from the loop upon `Escape`.
                    glium::glutin::WindowEvent::Closed
                    | glium::glutin::WindowEvent::KeyboardInput {
                        input:
                            glium::glutin::KeyboardInput {
                                virtual_keycode: Some(glium::glutin::VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => break 'main,
                    _ => (),
                },
                _ => (),
            }
        }

        const BUTTON_HEIGHT: f64 = 28.;
        const BUTTON_PAD: f64 = 4.;
        const BUTTON_PLUS_MINUS_WIDTH: f64 = 32.;

        // Instantiate the widgets.
        {
            let ui = &mut ui.set_widgets();

            widget::Canvas::new()
                .flow_down(&[
                    (
                        ids.buttons,
                        widget::Canvas::new()
                            .length(BUTTON_HEIGHT + BUTTON_PAD + BUTTON_PAD)
                            .color(color::LIGHT_BLUE)
                            .pad(BUTTON_PAD),
                    ),
                    (
                        ids.background,
                        widget::Canvas::new().border(0.0f64).color(color::CHARCOAL),
                    ),
                ])
                .set(ids.root, ui);

            for _ in widget::Button::new()
                .label("+")
                .mid_left_of(ids.buttons)
                .w_h(BUTTON_PLUS_MINUS_WIDTH, BUTTON_HEIGHT)
                .set(ids.fft_up, ui)
            {
                params.fft_width *= 2;
                event_loop.needs_update();
            }

            for _ in widget::Button::new()
                .label("-")
                .mid_left_of(ids.buttons)
                .w_h(BUTTON_PLUS_MINUS_WIDTH, BUTTON_HEIGHT)
                .right_from(ids.fft_up, BUTTON_PAD)
                .set(ids.fft_down, ui)
            {
                if params.fft_width > 2 {
                    params.fft_width /= 2;
                    event_loop.needs_update();
                }
            }

            widget::Text::new(&format!("fft: {}", params.fft_width))
                .mid_left_of(ids.buttons)
                .right_from(ids.fft_down, BUTTON_PAD)
                .w(64.)
                .set(ids.fft_label, ui);

            for _ in widget::Button::new()
                .label("+")
                .mid_left_of(ids.buttons)
                .w_h(BUTTON_PLUS_MINUS_WIDTH, BUTTON_HEIGHT)
                .right_from(ids.fft_label, BUTTON_PAD)
                .set(ids.stretch_up, ui)
            {
                params.stretch += 1;
                event_loop.needs_update();
            }

            for _ in widget::Button::new()
                .label("-")
                .mid_left_of(ids.buttons)
                .w_h(BUTTON_PLUS_MINUS_WIDTH, BUTTON_HEIGHT)
                .right_from(ids.stretch_up, BUTTON_PAD)
                .set(ids.stretch_down, ui)
            {
                params.stretch -= 1;
                event_loop.needs_update();
            }

            widget::Text::new(&format!("stretch: {}", params.stretch))
                .mid_left_of(ids.buttons)
                .right_from(ids.stretch_down, BUTTON_PAD)
                .w(128.)
                .set(ids.stretch_label, ui);

            for _ in widget::Button::new()
                .label("+")
                .mid_left_of(ids.buttons)
                .w_h(BUTTON_PLUS_MINUS_WIDTH, BUTTON_HEIGHT)
                .right_from(ids.stretch_label, BUTTON_PAD)
                .set(ids.stride_up, ui)
            {
                params.stride += 1;
                event_loop.needs_update();
            }

            for _ in widget::Button::new()
                .label("-")
                .mid_left_of(ids.buttons)
                .w_h(BUTTON_PLUS_MINUS_WIDTH, BUTTON_HEIGHT)
                .right_from(ids.stride_up, BUTTON_PAD)
                .set(ids.stride_down, ui)
            {
                if params.stride > 1 {
                    params.stride -= 1;
                    event_loop.needs_update();
                }
            }

            for val in widget::NumberDialer::new(f64::from(params.stride), 1., 4096., 0)
                .mid_left_of(ids.buttons)
                .right_from(ids.stride_down, BUTTON_PAD)
                .w(64.)
                .set(ids.stride_label, ui)
            {
                let val: f64 = val;
                params.stride = val.round() as u32;
            }

            if let Some((_, _, w, h)) = ui.kid_area_of(ids.background).map(|r| r.x_y_w_h()) {
                let w = w as u32;
                let h = h as u32;
                params.width = w;
                params.height = h;
                if params != prev_params || canvas_img.is_none() {
                    let datums = match render(samples, &params) {
                        Ok(datums) => datums,
                        Err(e) => {
                            println!("TODO: render failed: {:?}", e);
                            vec![(0, 0, 0); w as usize * h as usize]
                        }
                    };
                    let img = RawImage2d {
                        data: datums.into(),
                        width: w as u32,
                        height: h as u32,
                        format: ClientFormat::U8U8U8,
                    };
                    let img = glium::texture::Texture2d::new(&display, img).unwrap();

                    prev_params = params;

                    if let Some(id) = canvas_img {
                        image_map.replace(id, img);
                    } else {
                        canvas_img = Some(image_map.insert(img));
                    }
                }

                widget::Image::new(canvas_img.unwrap())
                    .w_h(w as f64, h as f64)
                    .top_left_of(ids.background)
                    .set(ids.canvas, ui);
            }
        }

        // Render the `Ui` and then display it on the screen.
        if let Some(primitives) = ui.draw_if_changed() {
            renderer.fill(&display, primitives, &image_map);
            let mut target = display.draw();
            target.clear_color(0.0, 0.0, 0.0, 1.0);
            renderer.draw(&display, &mut target, &image_map).unwrap();
            target.finish().unwrap();
        }
    }

    Ok(())
}

struct MemImage {
    data: Vec<(u8, u8, u8)>,
    width: usize,
    height: usize,
}

impl MemImage {
    #[inline]
    fn set(&mut self, x: usize, y: usize, val: (u8, u8, u8)) {
        assert!(x < self.width);
        assert!(y < self.height);

        self.data[(self.height - y - 1) * self.width + x] = val;
    }
}

fn render(samples: &mut Samples, params: &Params) -> Result<Vec<(u8, u8, u8)>, Error> {
    let w = params.width as usize;
    let h = params.height as usize;

    ensure!(w > params.fft_width, "TODO: window too narrow");

    let mut target = MemImage {
        data: vec![(0u8, 0u8, 0u8); w * h],
        width: w,
        height: h,
    };

    let fft = Radix4::<f32>::new(params.fft_width, false);

    ensure!(params.stretch > 0, "TODO: negative stretching");
    let stretch = params.stretch as usize;

    let mut sample_pos = 0;
    let mut ox = 0;
    let mut row = 0;

    let row_height = stretch * params.fft_width + 16;

    let mut min = 99.0;
    let mut max = 0.0;

    let scan = params.stride;
    let mut scan_pos = 0;
    let mut means = (0., 0.);

    let samples_available = samples.len() - params.fft_width as u64;
    while sample_pos < samples_available {
        let out = fft_at(&fft, samples, sample_pos)?;

        let oy = row * row_height;

        if oy > h {
            break;
        }

        means.0 += out
            .iter()
            .take(params.fft_width / 2)
            .map(|v| v.norm())
            .sum::<f32>();
        means.1 += out
            .iter()
            .skip(params.fft_width / 2)
            .map(|v| v.norm())
            .sum::<f32>();

        for (o, v) in out
            .iter()
            .skip(params.fft_width / 2)
            .chain(out.iter().take(params.fft_width / 2))
            .enumerate()
        {
            use palette::RgbHue;
            //let v = (v.norm() / 10.0 * 256.0) as u8;
            let scaled = v.norm() / 2.29;
            if scaled < min {
                min = scaled;
            }
            if scaled > max {
                max = scaled;
            }

            let scaled = 1.0 - scaled;

            let rgb = palette::rgb::Rgb::from(palette::Hsv::new(
                RgbHue::from(scaled * 0.8 * 360.0),
                1.0,
                1.0 - scaled,
            ));
            let mut v = (
                (rgb.red * 256.0) as u8,
                (rgb.green * 256.0) as u8,
                (rgb.blue * 256.0) as u8,
            );

            if 0 == scan_pos {
                v = (0, 0, 0);
            }

            for off in 0..stretch {
                let y = oy + o * stretch + off;
                if y >= h {
                    continue;
                }
                target.set(ox, y, v);
            }
        }

        ox += 1;
        if ox >= w {
            ox = 0;
            row += 1;
        }

        scan_pos += 1;
        if scan_pos >= scan {
            scan_pos = 0;
            #[cfg(never)]
            println!(
                "{}: {:.0} {:?}",
                if means.0 < means.1 { 0 } else { 1 },
                10. * (means.0 - means.1).abs() / means.0.max(means.1),
                means
            );
            means = (0., 0.);
        }

        sample_pos += 1;
    }

    println!("{} {}", min, max);

    Ok(target.data)
}

#[inline]
fn fft_at(
    fft: &Radix4<f32>,
    samples: &mut Samples,
    sample_pos: u64,
) -> Result<Vec<Complex<f32>>, Error> {
    use rustfft::num_traits::identities::Zero;
    use rustfft::Length;
    use rustfft::FFT;

    let fft_width = fft.len();
    let mut out = vec![Complex::zero(); fft_width];
    {
        let mut inp = vec![Complex::zero(); fft_width];
        samples.read_exact_at(sample_pos, &mut inp)?;
        fft.process(&mut inp, &mut out);
    }

    Ok(out)
}
