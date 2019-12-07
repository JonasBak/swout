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

#[derive(Serialize, Deserialize, Debug)]
struct InactiveOutput {
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct TmpOutput {
    id: Option<u16>,
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

fn get_outputs() -> Result<(Vec<Output>, Vec<InactiveOutput>)> {
    let raw_output = Command::new("swaymsg")
        .arg("-t")
        .arg("get_outputs")
        .output()
        .expect("failed to get outputs from swaymsg");
    let mut active_outputs: Vec<Output> = Vec::new();
    let mut inactive_outputs: Vec<InactiveOutput> = Vec::new();
    let outputs: Vec<TmpOutput> = serde_json::from_slice(&raw_output.stdout)?;
    for output in outputs.into_iter() {
        if let Some(id) = output.id {
            active_outputs.push(Output {
                id,
                name: output.name,
                rect: output.rect,
            });
        } else {
            inactive_outputs.push(InactiveOutput { name: output.name });
        }
    }

    return Ok((active_outputs, inactive_outputs));
}

fn set_active(output: &InactiveOutput) -> (Vec<Output>, Vec<InactiveOutput>) {
    Command::new("swaymsg")
        .arg(format!("output {} enable", output.name))
        .output()
        .expect("failed to get outputs from swaymsg");
    get_outputs().unwrap()
}

fn set_inactive(output: &Output) -> (Vec<Output>, Vec<InactiveOutput>) {
    Command::new("swaymsg")
        .arg(format!("output {} disable", output.name))
        .output()
        .expect("failed to get outputs from swaymsg");
    get_outputs().unwrap()
}

fn update_output_position(moved: MouseTracker, mut outputs: Vec<Output>) -> Vec<Output> {
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

    Command::new("swaymsg")
        .arg(command_arg.join(" "))
        .output()
        .expect("failed to set outputs");

    get_outputs().unwrap().0
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

fn check_inactive_touched(
    point: (i32, i32),
    inactive_size: u32,
    outputs: &[InactiveOutput],
) -> Option<&InactiveOutput> {
    for (i, output) in outputs.iter().enumerate() {
        if check_inside(
            point,
            &Bounds {
                x: i as i32 * inactive_size as i32,
                y: 0,
                width: inactive_size,
                height: inactive_size,
            },
        ) {
            return Some(output);
        }
    }
    None
}

fn handle_overlap(mut moved: MouseTracker, outputs: &[Output]) -> MouseTracker {
    let pos = (moved.pos.0 + moved.offset.0, moved.pos.1 + moved.offset.1);
    let center = (
        pos.0 + moved.size.0 as i32 / 2,
        pos.1 + moved.size.1 as i32 / 2,
    );
    let mut closest: Option<(i32, &Output)> = None;
    // Could do closest in x and y, to (maybe) handle corners better
    for output in outputs.iter() {
        if output.id != moved.id {
            let dist_sqr = (output.rect.x + output.rect.width as i32 / 2 - center.0).pow(2)
                + (output.rect.y + output.rect.height as i32 / 2 - center.1).pow(2);
            match closest {
                Some((d, _)) if dist_sqr <= d => {
                    closest = Some((dist_sqr, output));
                }
                None => {
                    closest = Some((dist_sqr, output));
                }
                _ => {}
            }
        }
    }
    if let Some((_, output)) = closest {
        let overlap_amount_x = moved.size.0 as i32 + output.rect.width as i32
            - ((pos.0 + moved.size.0 as i32).max(output.rect.x + output.rect.width as i32)
                - pos.0.min(output.rect.x));
        let overlap_amount_y = moved.size.1 as i32 + output.rect.height as i32
            - ((pos.1 + moved.size.1 as i32).max(output.rect.y + output.rect.height as i32)
                - pos.1.min(output.rect.y));
        if !(overlap_amount_x < 0 && overlap_amount_y < 0) {
            if overlap_amount_x < overlap_amount_y {
                moved.pos.0 += if center.0 < output.rect.x + output.rect.width as i32 / 2 {
                    -overlap_amount_x
                } else {
                    overlap_amount_x
                };
            } else {
                moved.pos.1 += if center.1 < output.rect.y + output.rect.height as i32 / 2 {
                    -overlap_amount_y
                } else {
                    overlap_amount_y
                };
            }
        }
    }
    moved
}

fn main() {
    let (mut active_outputs, mut inactive_outputs) = get_outputs().unwrap();

    let scale = 10.0;

    let inactive_size = 50 * scale as u32;

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

    let mut ctrl_down = false;

    'running: loop {
        let center = {
            let window = canvas.window_mut();
            let size = window.size();
            let output_bounds = active_outputs.iter().fold((0, 0, 0, 0), |a, b| {
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
                    moved.size.0,
                    moved.size.1,
                ))
                .unwrap();
        }
        // Render active
        for (i, output) in active_outputs.iter().enumerate() {
            let color = match selected {
                Some(MouseTracker { id, .. }) if id == output.id => selected_color,
                _ => colors[i],
            };
            canvas.set_draw_color(Color::RGB(color.0, color.1, color.2));
            canvas
                .fill_rect(Rect::new(
                    output.rect.x + center.0,
                    output.rect.y + center.1,
                    output.rect.width,
                    output.rect.height,
                ))
                .unwrap();
        }
        // Render inactive
        for (i, _) in inactive_outputs.iter().enumerate() {
            canvas.set_draw_color(Color::RGB(100, 0, 100));
            canvas
                .fill_rect(Rect::new(
                    inactive_size as i32 * i as i32,
                    0,
                    inactive_size as u32,
                    inactive_size as u32,
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
                Event::KeyDown {
                    keycode: Some(Keycode::LCtrl),
                    ..
                } => ctrl_down = true,
                Event::KeyUp {
                    keycode: Some(Keycode::LCtrl),
                    ..
                } => ctrl_down = false,
                Event::MouseButtonDown {
                    x: screen_x,
                    y: screen_y,
                    ..
                } => {
                    let x = screen_x * scale as i32 - center.0;
                    let y = screen_y * scale as i32 - center.1;
                    if ctrl_down {
                        if let Some(output) = check_inactive_touched(
                            (screen_x * scale as i32, screen_y * scale as i32),
                            inactive_size,
                            &inactive_outputs,
                        ) {
                            let (new_active_outputs, new_inactive_outputs) = set_active(&output);
                            active_outputs = new_active_outputs;
                            inactive_outputs = new_inactive_outputs;
                        } else if active_outputs.len() > 1 {
                            if let Some(output) = check_touched((x, y), &active_outputs) {
                                let (new_active_outputs, new_inactive_outputs) =
                                    set_inactive(&output);
                                active_outputs = new_active_outputs;
                                inactive_outputs = new_inactive_outputs;
                            }
                        }
                    } else {
                        selected = match check_touched((x, y), &active_outputs) {
                            Some(output) => Some(MouseTracker {
                                id: output.id,
                                offset: (output.rect.x - x, output.rect.y - y),
                                pos: (x, y),
                                size: (output.rect.width, output.rect.height),
                            }),
                            _ => None,
                        }
                    }
                }
                Event::MouseButtonUp { .. } => {
                    if let Some(moved) = selected.take() {
                        active_outputs = update_output_position(moved, active_outputs);
                    }
                }
                Event::MouseMotion {
                    x: screen_x,
                    y: screen_y,
                    ..
                } => {
                    let x = screen_x * scale as i32 - center.0;
                    let y = screen_y * scale as i32 - center.1;
                    if let Some(mut moved) = selected.take() {
                        moved.pos = (x, y);
                        selected = Some(handle_overlap(moved, &active_outputs));
                    }
                }
                _ => {}
            }
        }
        sleep(Duration::new(0, 1_000_000_000u32 / 30));
    }
}
