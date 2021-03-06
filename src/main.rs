#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unreachable_code)]
#![feature(vec_remove_item)]
extern crate cgmath;
extern crate gl;
extern crate glutin;
extern crate image;
extern crate uuid;

mod bounds;
mod color;
mod constants;
mod graph;
mod interaction;
mod network;
mod operator;
mod preview;
mod program;
mod renderer;
mod shader_builder;
mod texture;

// TODO:
// - Limit generators (i.e. sphere) to ONE output, since
//   the current graph traversal code doesn't work if the
//   same generator is connected to multiple other nodes.
//   the other option would be to properly handle this
//   during graph traversal so that the shader code for this
//   generator is duplicated. This would mean that transforms
//   should be their own family of operator as well.

use color::Color;
use interaction::{MouseInfo, Panel};
use operator::{DomainType, Op, OpFamily, Parameters, PrimitiveType};
use network::Network;
use preview::Shading;
use program::Program;
use renderer::Renderer;
use shader_builder::ShaderBuilder;

use glutin::GlContext;
use cgmath::{Vector2, Vector3, Vector4, Zero};

fn clear() {
    unsafe {
        let clear = Color::from_hex(constants::NETWORK_BACKGROUND_COLOR, constants::NETWORK_BACKGROUND_ALPHA);
        gl::ClearColor(clear.r, clear.g, clear.b, clear.a);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }
}

fn main() {
    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_dimensions(constants::WINDOW_RESOLUTION.x as u32, constants::WINDOW_RESOLUTION.y as u32)
        .with_title(constants::WINDOW_TITLE);
    let context = glutin::ContextBuilder::new().with_multisampling(constants::WINDOW_MULTISAMPLES);
    let gl_window = glutin::GlWindow::new(window, context, &events_loop).unwrap();
    unsafe { gl_window.make_current() }.unwrap();
    gl::load_with(|symbol| gl_window.get_proc_address(symbol) as *const _);

    // Keep track of the current window size and interaction state
    let mut current_size = Vector2::new(constants::WINDOW_RESOLUTION.x, constants::WINDOW_RESOLUTION.y);
    let mut mouse = MouseInfo::new();

    // Main objects
    let mut network = Network::new(current_size);
    let mut builder = ShaderBuilder::new();

    loop {
        events_loop.poll_events(|event| {
            match event {
                glutin::Event::WindowEvent { event, .. } => match event {
                    glutin::WindowEvent::Closed => (),

                    glutin::WindowEvent::Resized(w, h) => {
                        current_size = Vector2 { x: w as f32, y: h as f32 };
                        gl_window.resize(w, h);
                    }

                    glutin::WindowEvent::MouseMoved { position, .. } => {
                        mouse.last = mouse.curr;
                        mouse.curr = Vector2::new(position.0 as f32, position.1 as f32);

                        // Zero center and zoom.
                        mouse.curr -= current_size * 0.5;
                        //mouse.curr *= mouse.scroll;

                        network.handle_interaction(&mouse);
                    }

                    glutin::WindowEvent::MouseWheel { delta, .. } => {
                        if let glutin::MouseScrollDelta::LineDelta(_, line_y) = delta {
                            if line_y == 1.0 {
                                mouse.scroll -= constants::ZOOM_INCREMENT;
                            } else {
                                mouse.scroll += constants::ZOOM_INCREMENT;
                            }
                            network.handle_interaction(&mouse);
                        }
                    }

                    glutin::WindowEvent::MouseInput { state, button, .. } => {
                        if let glutin::ElementState::Pressed = state {
                            // Store the current mouse position.
                            mouse.clicked = mouse.curr;

                            // Store mouse button presses.
                            match button {
                                glutin::MouseButton::Left => mouse.ldown = true,
                                glutin::MouseButton::Right => mouse.rdown = true,
                                glutin::MouseButton::Middle => mouse.mdown = true,
                                _ => (),
                            }
                            network.handle_interaction(&mouse);
                        } else {
                            mouse.ldown = false;
                            mouse.rdown = false;
                            mouse.mdown = false;
                        }
                    }

                    glutin::WindowEvent::KeyboardInput { input, .. } => {
                        if let glutin::ElementState::Pressed = input.state {
                            if let Some(key) = input.virtual_keycode {
                                if input.modifiers.shift && key != glutin::VirtualKeyCode::LShift {
                                    // If the `shift` modifier is down, add a new op.
                                    let family = match key {
                                        glutin::VirtualKeyCode::S => {
                                            Some(OpFamily::Primitive(PrimitiveType::Sphere))
                                        }
                                        glutin::VirtualKeyCode::B => {
                                            Some(OpFamily::Primitive(PrimitiveType::Box))
                                        }
                                        glutin::VirtualKeyCode::P => {
                                            Some(OpFamily::Primitive(PrimitiveType::Plane))
                                        }
                                        glutin::VirtualKeyCode::T => {
                                            Some(OpFamily::Primitive(PrimitiveType::Torus))
                                        }
                                        glutin::VirtualKeyCode::U => {
                                            Some(OpFamily::Primitive(PrimitiveType::Union))
                                        }
                                        glutin::VirtualKeyCode::D => {
                                            Some(OpFamily::Primitive(PrimitiveType::Subtraction))
                                        }
                                        glutin::VirtualKeyCode::I => {
                                            Some(OpFamily::Primitive(PrimitiveType::Intersection))
                                        }
                                        glutin::VirtualKeyCode::M => {
                                            Some(OpFamily::Primitive(PrimitiveType::SmoothMinimum))
                                        }
                                        glutin::VirtualKeyCode::R => {
                                            Some(OpFamily::Primitive(PrimitiveType::Render))
                                        }
                                        glutin::VirtualKeyCode::Key1 => {
                                            Some(OpFamily::Domain(DomainType::Root))
                                        }
                                        glutin::VirtualKeyCode::Key2 => {
                                            Some(OpFamily::Domain(DomainType::Transform))
                                        }
                                        glutin::VirtualKeyCode::Key3 => {
                                            Some(OpFamily::Domain(DomainType::Twist))
                                        }
                                        glutin::VirtualKeyCode::Key4 => {
                                            Some(OpFamily::Domain(DomainType::Bend))
                                        }
                                        _ => None,
                                    };
                                    if let Some(family) = family {
                                        network.add_op(
                                            family,
                                            mouse.curr - constants::OPERATOR_SIZE * 0.5,
                                            constants::OPERATOR_SIZE,
                                        );
                                    }
                                } else {
                                    // Handle other key commands.
                                    match key {
                                        glutin::VirtualKeyCode::Delete => network.delete_selected(),
                                        glutin::VirtualKeyCode::H => {
                                            mouse.scroll = 1.0;
                                            network.preview.home();
                                        }
                                        glutin::VirtualKeyCode::P => network.toggle_preview(),
                                        glutin::VirtualKeyCode::Key1 => {
                                            network.preview.set_shading(Shading::Depth)
                                        }
                                        glutin::VirtualKeyCode::Key2 => {
                                            network.preview.set_shading(Shading::Steps)
                                        }
                                        glutin::VirtualKeyCode::Key3 => {
                                            network.preview.set_shading(Shading::AmbientOcclusion)
                                        }
                                        glutin::VirtualKeyCode::Key4 => {
                                            network.preview.set_shading(Shading::Normals)
                                        }
                                        glutin::VirtualKeyCode::Key5 => {
                                            network.preview.set_shading(Shading::Diffuse)
                                        }
                                        glutin::VirtualKeyCode::Equals => {
                                            network.increment_param(&Vector4::new(
                                                0.0,
                                                0.0,
                                                0.0,
                                                0.05,
                                            ));
                                        }
                                        glutin::VirtualKeyCode::Minus => {
                                            network.increment_param(&Vector4::new(
                                                0.0,
                                                0.0,
                                                0.0,
                                                -0.05,
                                            ));
                                        }
                                        glutin::VirtualKeyCode::Left => {
                                            network.increment_param(&Vector4::new(
                                                0.05,
                                                0.0,
                                                0.0,
                                                0.0,
                                            ));
                                        }
                                        glutin::VirtualKeyCode::Right => {
                                            network.increment_param(&Vector4::new(
                                                -0.05,
                                                0.0,
                                                0.0,
                                                0.0,
                                            ));
                                        }
                                        glutin::VirtualKeyCode::Up => {
                                            network.increment_param(&Vector4::new(
                                                0.0,
                                                -0.05,
                                                0.0,
                                                0.0,
                                            ));
                                        }
                                        glutin::VirtualKeyCode::Down => {
                                            network.increment_param(&Vector4::new(
                                                0.0,
                                                0.05,
                                                0.0,
                                                0.0,
                                            ));
                                        }
                                        _ => (),
                                    }
                                }
                            }
                        }
                    }
                    _ => (),
                },
                _ => (),
            }
        });

        clear();

        // Check to see if the graph needs to be rebuilt.
        if network.dirty() {
            if let Some(root) = network.render_id {
                let indices = network.graph.traverse(root);
                let program = builder.build_sources(&network, indices);
                network.preview.set_valid_program(program);
                network.clean();
            } else {
                network.preview.set_valid_program(None);
            }
        }

        // Draw the graph (ops, connections, preview window, etc.).
        network.draw();

        gl_window.swap_buffers().unwrap();
    }
}
