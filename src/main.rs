extern crate sdl2;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;

use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
struct Bounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Output {
    id: u16,
    name: String,
    rect: Bounds,
}

#[derive(Debug)]
struct MouseTracker {
    id: u16,
    pos: (i32, i32),
    offset: (i32, i32),
    size: (u32, u32),
}

fn get_outputs() -> Result<Vec<Output>> {
    let raw_output = Command::new("swaymsg")
        .arg("-t")
        .arg("get_outputs")
        .output()
        .expect("failed to get outputs from swaymsg");
    let outputs: Vec<Output> = serde_json::from_slice(&raw_output.stdout)?;
    return Ok(outputs);
}

fn set_outputs(moved: MouseTracker, mut outputs: Vec<Output>) -> Vec<Output> {
    let mut min_x = moved.pos.0;
    let mut min_y = moved.pos.1;
    for output in outputs.iter_mut() {
        if output.id == moved.id {
            output.rect.x = moved.pos.0 + moved.offset.0;
            output.rect.y = moved.pos.1 + moved.offset.1;
        }
        min_x = min_x.min(output.rect.x);
        min_y = min_y.min(output.rect.y);
    }
    for output in outputs.iter_mut() {
        output.rect.x -= min_x;
        output.rect.y -= min_y;
    }

    let command_arg: Vec<String> = outputs
        .iter()
        .map(|output| {
            format!(
                "output {} pos {} {};",
                output.name, output.rect.x, output.rect.y
            )
        })
        .collect();
    println!("{}", command_arg.join(" "));
    Command::new("swaymsg")
        .arg(command_arg.join(" "))
        .output()
        .expect("failed to get outputs from swaymsg");

    outputs
}

fn check_inside(point: (i32, i32), bounds: &Bounds) -> bool {
    (bounds.x..=bounds.x + bounds.width as i32).contains(&point.0)
        && (bounds.y..=bounds.y + bounds.height as i32).contains(&point.1)
}

fn check_touched(point: (i32, i32), outputs: &[Output]) -> Option<&Output> {
    for output in outputs.iter() {
        if check_inside(point, &output.rect) {
            return Some(output);
        }
    }
    None
}

fn main() {
    let mut outputs = get_outputs().unwrap();

    let scale = 10.0;

    let colors = vec![(255, 0, 0), (0, 255, 0), (0, 0, 255)];
    let selected_color = (200, 200, 200);
    let moved_color = (100, 100, 100);

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("rust-sdl2 demo: Video", 800, 600)
        .position_centered()
        .opengl()
        .resizable()
        .build()
        .map_err(|e| e.to_string())
        .unwrap();

    let mut canvas = window
        .into_canvas()
        .build()
        .map_err(|e| e.to_string())
        .unwrap();

    canvas.set_scale(1.0 / scale, 1.0 / scale).unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut selected: Option<MouseTracker> = None;

    'running: loop {
        // Get offset to center in "screen coordinates"
        let center = {
            let window = canvas.window_mut();
            let size = window.size();
            let output_bounds = outputs.iter().fold((0, 0, 0, 0), |a, b| {
                (
                    a.0.min(b.rect.x),
                    a.1.min(b.rect.y),
                    a.2.max(b.rect.x + b.rect.width as i32),
                    a.3.max(b.rect.y + b.rect.height as i32),
                )
            });
            (
                (size.0 * scale as u32 / 2) as i32 - (output_bounds.0 + output_bounds.2) / 2,
                (size.1 * scale as u32 / 2) as i32 - (output_bounds.1 + output_bounds.3) / 2,
            )
        };

        canvas.set_draw_color(Color::RGB(50, 50, 50));
        canvas.clear();

        // Render selected
        if let Some(ref moved) = selected {
            canvas.set_draw_color(Color::RGB(moved_color.0, moved_color.1, moved_color.2));
            canvas
                .fill_rect(Rect::new(
                    moved.pos.0 + moved.offset.0 + center.0,
                    moved.pos.1 + moved.offset.1 + center.1,
                    moved.size.0 as u32,
                    moved.size.1 as u32,
                ))
                .unwrap();
        }
        // Render all
        for (i, output) in outputs.iter().enumerate() {
            let color = match selected {
                Some(MouseTracker { id, .. }) if id == output.id => selected_color,
                _ => colors[i],
            };
            canvas.set_draw_color(Color::RGB(color.0, color.1, color.2));
            canvas
                .fill_rect(Rect::new(
                    output.rect.x + center.0,
                    output.rect.y + center.1,
                    output.rect.width as u32,
                    output.rect.height as u32,
                ))
                .unwrap();
        }
        canvas.present();

        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::MouseButtonDown { x, y, .. } => {
                    let x = x - center.0;
                    let y = y - center.1;
                    selected = match check_touched((x, y), &outputs) {
                        Some(output) => Some(MouseTracker {
                            id: output.id,
                            offset: (output.rect.x - x, output.rect.y - y),
                            pos: (x, y),
                            size: (output.rect.width, output.rect.height),
                        }),
                        _ => None,
                    }
                }
                Event::MouseButtonUp { .. } => {
                    if let Some(moved) = selected.take() {
                        outputs = set_outputs(moved, outputs);
                    }
                }
                Event::MouseMotion { x, y, .. } => {
                    let x = x - center.0;
                    let y = y - center.1;
                    if let Some(mut moved) = selected.take() {
                        moved.pos = (x, y);
                        selected = Some(moved);
                    }
                }
                _ => {}
            }
        }
        sleep(Duration::new(0, 1_000_000_000u32 / 30));
    }
}
