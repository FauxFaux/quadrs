use conrod::{self, color, widget, Colorable, Positionable, Sizeable, Widget};
use conrod::backend::glium::glium;
use conrod::backend::glium::glium::Surface;
use conrod::Borderable;

use self::glium::texture::ClientFormat;
use self::glium::texture::RawImage2d;

use palette;
use rustfft::algorithm::Radix4;
use rustfft::num_complex::Complex;

use errors::*;
use samples::Samples;

mod support;

pub fn display(samples: &mut Samples) -> Result<()> {
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

    // A type used for converting `conrod::render::Primitives` into `Command`s that can be used
    // for drawing to the glium `Surface`.
    let mut renderer = conrod::backend::glium::Renderer::new(&display).unwrap();

    // The `WidgetId` for our background and `Image` widgets.
    widget_ids!(struct Ids {
        background,
        background_scrollbar,
        canvas,
        buttons,
    });
    let ids = Ids::new(ui.widget_id_generator());

    let mut prev_dims = (0, 0);

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

        // Instantiate the widgets.
        {
            let ui = &mut ui.set_widgets();

            widget::Canvas::new()
                .top_left()
                .border(0.0f64)
                .color(color::CHARCOAL)
                .set(ids.background, ui);

            widget::Scrollbar::y_axis(ids.background).set(ids.background_scrollbar, ui);

            if let Some((_, _, w, h)) = ui.kid_area_of(ids.background).map(|r| r.x_y_w_h()) {
                let w = w as u32;
                let h = h as u32;
                if (w, h) != prev_dims || canvas_img.is_none() {
                    let datums = render(samples, w, h)?;
                    let img = RawImage2d {
                        data: datums.into(),
                        width: w as u32,
                        height: h as u32,
                        format: ClientFormat::U8U8U8,
                    };
                    let img = glium::texture::Texture2d::new(&display, img).unwrap();

                    prev_dims = (w, h);

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

fn render(samples: &mut Samples, w: u32, h: u32) -> Result<Vec<(u8, u8, u8)>> {
    let w = w as usize;
    let h = h as usize;

    let stride = 1;
    let fft_width = 8;

    ensure!(w > fft_width, "TODO: window too narrow");

    let mut target = MemImage {
        data: vec![(0u8, 0u8, 0u8); w * h],
        width: w,
        height: h,
    };

    let fft = Radix4::<f32>::new(fft_width, false);

    let stretch = 16;

    let mut sample_pos = 0;
    let mut ox = 0;
    let mut row = 0;

    let row_height = stretch * fft_width + 16;

    let mut min = 99.0;
    let mut max = 0.0;

    let samples_available = samples.len() - fft_width as u64;
    while sample_pos < samples_available {
        let out = fft_at(&fft, samples, sample_pos)?;

        let oy = row * row_height;

        if oy > h {
            break;
        }

        for (o, v) in out.iter()
            .skip(fft_width / 2)
            .chain(out.iter().take(fft_width / 2))
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

            let rgb = palette::Rgb::from(palette::Hsv::new(
                RgbHue::from(scaled * 0.8 * 360.0),
                1.0,
                1.0 - scaled,
            ));
            let v = (
                (rgb.red * 256.0) as u8,
                (rgb.green * 256.0) as u8,
                (rgb.blue * 256.0) as u8,
            );
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
        sample_pos += stride;
    }

    println!("{} {}", min, max);

    Ok(target.data)
}

#[inline]
fn fft_at(fft: &Radix4<f32>, samples: &mut Samples, sample_pos: u64) -> Result<Vec<Complex<f32>>> {
    use rustfft::FFT;
    use rustfft::Length;
    use rustfft::num_traits::identities::Zero;

    let fft_width = fft.len();
    let mut out = vec![Complex::zero(); fft_width];
    {
        let mut inp = vec![Complex::zero(); fft_width];
        samples.read_exact_at(sample_pos, &mut inp)?;
        fft.process(&mut inp, &mut out);
    }

    Ok(out)
}
