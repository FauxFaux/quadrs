use std::mem;

use conrod::{self, color, widget, Colorable, Positionable, Sizeable, Widget};
use conrod::backend::glium::glium;
use conrod::backend::glium::glium::Surface;
use conrod::Borderable;

use self::glium::texture::ClientFormat;
use self::glium::texture::RawImage2d;

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

fn render(samples: &mut Samples, w: u32, h: u32) -> Result<Vec<(u8, u8, u8)>> {
    use rustfft::FFT;
    use rustfft::algorithm::Radix4;
    use rustfft::num_complex::Complex;
    use rustfft::num_traits::identities::Zero;

    let w = w as usize;
    let h = h as usize;

    let stride = 4;
    let fft_width = 16;

    ensure!(w > fft_width, "TODO: window too narrow");

    let mut datums = vec![(0u8, 0u8, 0u8); w * h];

    let fft = Radix4::new(fft_width as usize, false);

    let mut i = 0;
    let mut oh = 0;
    while i < (samples.len() - fft_width as u64) && oh < h {
        let mut inp = vec![Complex::zero(); fft_width];
        samples.read_exact_at(i, &mut inp)?;

        let mut out = vec![Complex::zero(); fft_width];

        fft.process(&mut inp, &mut out);
        mem::drop(inp); // inp is now junk

        for (o, v) in out.iter()
            .skip(fft_width / 2)
            .chain(out.iter().take(fft_width / 2))
            .enumerate()
        {
            let v = (v.norm() / 10.0 * 256.0) as u8;
            datums[oh * w + o] = (v, v, v);
        }

        oh += 1;
        i += stride;
    }

    Ok(datums)
}
