#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pdf_processor;
mod rules;
mod config;
mod icon_generator;
mod logger;
mod file_watcher;
mod tray_app;
mod autostart;

use winit::event_loop::{EventLoop, ControlFlow};
use tray_app::TrayApp;

fn main() {
    // Initialize logging
    logger::init_logger();

    // Create event loop
    let event_loop = EventLoop::new().expect("Nie można utworzyć event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    // Create and run tray application
    let mut app = TrayApp::new();

    event_loop.run_app(&mut app).expect("Błąd event loop");
}
