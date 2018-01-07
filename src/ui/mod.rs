use std;

use conrod::{self, color, widget, Colorable, Positionable, Sizeable, Widget};
use conrod::backend::glium::glium;
use conrod::backend::glium::glium::Surface;
use conrod::Borderable;

use self::glium::texture::ClientFormat;
use self::glium::texture::RawImage2d;

mod support;

pub fn display() {
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
        rust_logo,
        buttons,
    });
    let ids = Ids::new(ui.widget_id_generator());

    let (w, h) = (128, 2000);
    let mut datums = vec![(0u8, 0u8, 0u8); (w * h) as usize];
    for i in 0..h {
        for j in 0..w {
            let val = (256. * (i as f32 / h as f32)) as u8;
            datums[(j + (i * w)) as usize] = (val, val, val);
        }
    }

    let rust_logo = RawImage2d {
        data: datums.into(),
        width: w,
        height: h,
        format: ClientFormat::U8U8U8,
    };
    let rust_logo = glium::texture::Texture2d::new(&display, rust_logo).unwrap();
    let mut image_map = conrod::image::Map::<glium::texture::Texture2d>::new();
    let gradient = image_map.insert(rust_logo);

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
                .top_left_with_margins(32. + 4., 0.)
                .border(0.0f64)
                .color(color::CHARCOAL)
                .scroll_kids_vertically()
                .set(ids.background, ui);

            widget::Canvas::new()
                .top_left()
                .h(32. + 4.)
                .color(color::LIGHT_BLUE)
                .set(ids.buttons, ui);

            widget::Scrollbar::y_axis(ids.background)
                .set(ids.background_scrollbar, ui);

            //println!("{:?}", ui.kid_area_of(ids.background));
            widget::Image::new(gradient)
                .w_h(w as f64, h as f64)
                .middle_of(ids.background)
                .set(ids.rust_logo, ui);
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
}
