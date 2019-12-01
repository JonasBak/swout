use serde::{Deserialize, Serialize};
use serde_json::{Result, Value};
use std::io::{stdin, stdout, Write};
use std::process::Command;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

#[derive(Serialize, Deserialize, Debug)]
struct Rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Output {
    id: u16,
    name: String,
    rect: Rect,
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

fn box_around<W: Write>(stdout: &mut W, string: &String, x: u16, y: u16, width: u16, height: u16) {
    write!(stdout, "{}", termion::cursor::Goto(x, y),).unwrap();
    write!(stdout, "{:width$}", "", width = width as usize).unwrap();
    write!(stdout, "\n{}", termion::cursor::Left(width)).unwrap();
    write!(stdout, "{:width$}", string, width = width as usize).unwrap();
    for _ in 0..height - 3 {
        write!(stdout, "\n{}", termion::cursor::Left(width)).unwrap();
        write!(stdout, "{:width$}", "", width = width as usize).unwrap();
    }
}

fn render_layout<W: Write>(stdout: &mut W, outputs: &Vec<Output>) {
    write!(
        stdout,
        "{}{}",
        termion::cursor::Goto(1, 1),
        termion::clear::All
    )
    .unwrap();

    let cell_x = 100;
    let cell_y = 100;

    let colors = vec![(255, 0, 0), (0, 255, 0), (0, 0, 255)];

    for (i, output) in outputs.iter().enumerate() {
        write!(
            stdout,
            "{}",
            termion::color::Bg(termion::color::Rgb(colors[i].0, colors[i].1, colors[i].2))
        )
        .unwrap();
        box_around(
            stdout,
            &output.name,
            (output.rect.x / cell_x) as u16 + 1,
            (output.rect.y / cell_y) as u16 + 1,
            (output.rect.width / cell_x) as u16 + 1,
            (output.rect.height / cell_y) as u16 + 1,
        );
    }
}

fn main() {
    let mut stdout = stdout().into_raw_mode().unwrap();
    let outputs = get_outputs().unwrap();
    render_layout(&mut stdout, &outputs);
}
