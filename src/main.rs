extern crate ccsds_primary_header;

extern crate bytes;
extern crate byteorder;

extern crate num;
#[macro_use] extern crate num_derive;

extern crate serde;
extern crate serde_json;
#[macro_use] extern crate serde_derive;

#[macro_use] extern crate log;
extern crate simplelog;

extern crate chrono;

extern crate floating_duration;

extern crate sdl2;
extern crate imgui;
extern crate imgui_sdl2;
extern crate gl;
extern crate imgui_opengl_renderer;


use std::time;
use std::time::Duration;
use std::thread;
use std::io::{Write, Read};
use std::default::Default;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::fs::File;
use std::fs::create_dir;

use ccsds_primary_header::*;

use byteorder::{LittleEndian, BigEndian};

use simplelog::*;

use chrono::prelude::*;

use floating_duration::TimeAsFloat;

use imgui::*;

mod stream;
use stream::*;

mod types;
use types::*;

mod processing;
use processing::*;


/// Window width given to SDL
const WINDOW_WIDTH:  f32 = 640.0;

/// Window height given to SDL
const WINDOW_HEIGHT: f32 = 740.0;


fn main() {
    // Set Up Logging
    create_dir("log");

    let date = Local::now();
    let log_name = format!("{}", date.format("log/ccsds_router_log_%Y%m%d_%H_%M_%S.log"));
    let logger = CombinedLogger::init(vec!(TermLogger::new(LevelFilter::max(),   Config::default()).unwrap(),
                                           WriteLogger::new(LevelFilter::max(), Config::default(), File::create(log_name).unwrap())
                                           )
                                      ).unwrap();


    // Spawn processing thread
    let (gui_sender,  gui_receiver)  = channel::<GuiMessage>();
    let (proc_sender, proc_receiver) = channel::<ProcessingMsg>();

    let ccsds_thread = thread::spawn(move || {
        process_thread( gui_sender, proc_receiver );
    });


    // Run GUI main loop
    run_gui( gui_receiver, proc_sender );


    // Clean up and Exit 
    ccsds_thread.join().unwrap();

    info!("Exiting");
}

// dark theme from codz01 (https://github.com/ocornut/imgui/issues/707)
fn set_style_dark(style: &mut ImGuiStyle) {
    style.frame_border_size = 1.0;
    style.frame_padding = ImVec2::new(4.0,2.0);
    style.item_spacing = ImVec2::new(8.0,2.0);
    style.window_border_size = 1.0;
    //style.tab_border_size = 1.0;
    style.window_rounding = 1.0;
    style.child_rounding = 1.0;
    style.frame_rounding = 1.0;
    style.scrollbar_rounding = 1.0;
    style.grab_rounding = 1.0;

    style.colors =
        [
        ImVec4::new(1.00, 1.00, 1.00, 0.95), // ImGuiCol_Text 
        ImVec4::new(0.50, 0.50, 0.50, 1.00), // ImGuiCol_TextDisabled 
        ImVec4::new(0.13, 0.12, 0.12, 1.00), // ImGuiCol_WindowBg 
        ImVec4::new(1.00, 1.00, 1.00, 0.00), // ImGuiCol_ChildBg 
        ImVec4::new(0.05, 0.05, 0.05, 0.94), // ImGuiCol_PopupBg 
        ImVec4::new(0.53, 0.53, 0.53, 0.46), // ImGuiCol_Border 
        ImVec4::new(0.00, 0.00, 0.00, 0.00), // ImGuiCol_BorderShadow 
        ImVec4::new(0.00, 0.00, 0.00, 0.85), // ImGuiCol_FrameBg 
        ImVec4::new(0.22, 0.22, 0.22, 0.40), // ImGuiCol_FrameBgHovered 
        ImVec4::new(0.16, 0.16, 0.16, 0.53), // ImGuiCol_FrameBgActive 
        ImVec4::new(0.00, 0.00, 0.00, 1.00), // ImGuiCol_TitleBg 
        ImVec4::new(0.00, 0.00, 0.00, 1.00), // ImGuiCol_TitleBgActive 
        ImVec4::new(0.00, 0.00, 0.00, 0.51), // ImGuiCol_TitleBgCollapsed 
        ImVec4::new(0.12, 0.12, 0.12, 1.00), // ImGuiCol_MenuBarBg 
        ImVec4::new(0.02, 0.02, 0.02, 0.53), // ImGuiCol_ScrollbarBg 
        ImVec4::new(0.31, 0.31, 0.31, 1.00), // ImGuiCol_ScrollbarGrab 
        ImVec4::new(0.41, 0.41, 0.41, 1.00), // ImGuiCol_ScrollbarGrabHovered 
        ImVec4::new(0.48, 0.48, 0.48, 1.00), // ImGuiCol_ScrollbarGrabActive 
        ImVec4::new(0.79, 0.79, 0.79, 1.00), // ImGuiCol_CheckMark 
        ImVec4::new(0.48, 0.47, 0.47, 0.91), // ImGuiCol_SliderGrab 
        ImVec4::new(0.56, 0.55, 0.55, 0.62), // ImGuiCol_SliderGrabActive 
        ImVec4::new(0.50, 0.50, 0.50, 0.63), // ImGuiCol_Button 
        ImVec4::new(0.67, 0.67, 0.68, 0.63), // ImGuiCol_ButtonHovered 
        ImVec4::new(0.26, 0.26, 0.26, 0.63), // ImGuiCol_ButtonActive 
        ImVec4::new(0.54, 0.54, 0.54, 0.58), // ImGuiCol_Header 
        ImVec4::new(0.64, 0.65, 0.65, 0.80), // ImGuiCol_HeaderHovered 
        ImVec4::new(0.25, 0.25, 0.25, 0.80), // ImGuiCol_HeaderActive 
        ImVec4::new(0.58, 0.58, 0.58, 0.50), // ImGuiCol_Separator 
        ImVec4::new(0.81, 0.81, 0.81, 0.64), // ImGuiCol_SeparatorHovered 
        ImVec4::new(0.81, 0.81, 0.81, 0.64), // ImGuiCol_SeparatorActive 
        ImVec4::new(0.87, 0.87, 0.87, 0.53), // ImGuiCol_ResizeGrip 
        ImVec4::new(0.87, 0.87, 0.87, 0.74), // ImGuiCol_ResizeGripHovered 
        ImVec4::new(0.87, 0.87, 0.87, 0.74), // ImGuiCol_ResizeGripActive 
        ImVec4::new(0.61, 0.61, 0.61, 1.00), // ImGuiCol_PlotLines 
        ImVec4::new(0.68, 0.68, 0.68, 1.00), // ImGuiCol_PlotLinesHovered 
        ImVec4::new(0.90, 0.77, 0.33, 1.00), // ImGuiCol_PlotHistogram 
        ImVec4::new(0.87, 0.55, 0.08, 1.00), // ImGuiCol_PlotHistogramHovered 
        ImVec4::new(0.47, 0.60, 0.76, 0.47), // ImGuiCol_TextSelectedBg 
        ImVec4::new(0.58, 0.58, 0.58, 0.90), // ImGuiCol_DragDropTarget 
        ImVec4::new(0.60, 0.60, 0.60, 1.00), // ImGuiCol_NavHighlight 
        ImVec4::new(1.00, 1.00, 1.00, 0.70), // ImGuiCol_NavWindowingHighlight 
        ImVec4::new(0.80, 0.80, 0.80, 0.20), // ImGuiCol_NavWindowingDimBg 
        ImVec4::new(0.80, 0.80, 0.80, 0.35), // ImGuiCol_ModalWindowDimBg 
        ];
}

// light green from @ebachard (https://github.com/ocornut/imgui/issues/707)
fn set_style_light(style: &mut ImGuiStyle) {
    style.window_rounding     = 2.0;
    style.scrollbar_rounding  = 3.0;
    style.grab_rounding       = 2.0;
    style.anti_aliased_lines  = true;
    style.anti_aliased_fill   = true;
    style.window_rounding     = 2.0;
    style.child_rounding      = 2.0;
    style.scrollbar_size      = 16.0;
    style.scrollbar_rounding  = 3.0;
    style.grab_rounding       = 2.0;
    style.item_spacing.x      = 10.0;
    style.item_spacing.y      = 4.0;
    style.indent_spacing      = 22.0;
    style.frame_padding.x     = 6.0;
    style.frame_padding.y     = 4.0;
    style.alpha               = 1.0;
    style.frame_rounding      = 3.0;

    style.colors =
        [
        ImVec4::new(0.00, 0.00, 0.00, 1.00), // ImGuiCol_Text
        ImVec4::new(0.60, 0.60, 0.60, 1.00), // ImGuiCol_TextDisabled
        ImVec4::new(0.86, 0.86, 0.86, 1.00), // ImGuiCol_WindowBg
        ImVec4::new(0.00, 0.00, 0.00, 0.00), // ImGuiCol_ChildBg
        ImVec4::new(0.93, 0.93, 0.93, 0.98), // ImGuiCol_PopupBg
        ImVec4::new(0.71, 0.71, 0.71, 0.08), // ImGuiCol_Border
        ImVec4::new(0.00, 0.00, 0.00, 0.04), // ImGuiCol_BorderShadow
        ImVec4::new(0.71, 0.71, 0.71, 0.55), // ImGuiCol_FrameBg
        ImVec4::new(0.94, 0.94, 0.94, 0.55), // ImGuiCol_FrameBgHovered
        ImVec4::new(0.71, 0.78, 0.69, 0.98), // ImGuiCol_FrameBgActive
        ImVec4::new(0.85, 0.85, 0.85, 1.00), // ImGuiCol_TitleBg
        ImVec4::new(0.78, 0.78, 0.78, 1.00), // ImGuiCol_TitleBgActive
        ImVec4::new(0.82, 0.78, 0.78, 0.51), // ImGuiCol_TitleBgCollapsed
        ImVec4::new(0.86, 0.86, 0.86, 1.00), // ImGuiCol_MenuBarBg
        ImVec4::new(0.20, 0.25, 0.30, 0.61), // ImGuiCol_ScrollbarBg
        ImVec4::new(0.90, 0.90, 0.90, 0.30), // ImGuiCol_ScrollbarGrab
        ImVec4::new(0.92, 0.92, 0.92, 0.78), // ImGuiCol_ScrollbarGrabHovered
        ImVec4::new(1.00, 1.00, 1.00, 1.00), // ImGuiCol_ScrollbarGrabActive
        ImVec4::new(0.184, 0.407, 0.193, 1.00), // ImGuiCol_CheckMark
        ImVec4::new(0.26, 0.59, 0.98, 0.78), // ImGuiCol_SliderGrab
        ImVec4::new(0.26, 0.59, 0.98, 1.00), // ImGuiCol_SliderGrabActive
        ImVec4::new(0.71, 0.78, 0.69, 0.40), // ImGuiCol_Button
        ImVec4::new(0.725, 0.805, 0.702, 1.00), // ImGuiCol_ButtonHovered
        ImVec4::new(0.793, 0.900, 0.836, 1.00), // ImGuiCol_ButtonActive
        ImVec4::new(0.71, 0.78, 0.69, 0.31), // ImGuiCol_Header
        ImVec4::new(0.71, 0.78, 0.69, 0.80), // ImGuiCol_HeaderHovered
        ImVec4::new(0.71, 0.78, 0.69, 1.00), // ImGuiCol_HeaderActive
        ImVec4::new(0.39, 0.39, 0.39, 1.00), // ImGuiCol_Separator
        ImVec4::new(0.14, 0.44, 0.80, 0.78), // ImGuiCol_SeparatorHovered
        ImVec4::new(0.14, 0.44, 0.80, 1.00), // ImGuiCol_SeparatorActive
        ImVec4::new(1.00, 1.00, 1.00, 0.00), // ImGuiCol_ResizeGrip
        ImVec4::new(0.26, 0.59, 0.98, 0.45), // ImGuiCol_ResizeGripHovered
        ImVec4::new(0.26, 0.59, 0.98, 0.78), // ImGuiCol_ResizeGripActive
        ImVec4::new(0.39, 0.39, 0.39, 1.00), // ImGuiCol_PlotLines
        ImVec4::new(1.00, 0.43, 0.35, 1.00), // ImGuiCol_PlotLinesHovered
        ImVec4::new(0.90, 0.70, 0.00, 1.00), // ImGuiCol_PlotHistogram
        ImVec4::new(1.00, 0.60, 0.00, 1.00), // ImGuiCol_PlotHistogramHovered
        ImVec4::new(0.26, 0.59, 0.98, 0.35), // ImGuiCol_TextSelectedBg
        ImVec4::new(0.26, 0.59, 0.98, 0.95), // ImGuiCol_DragDropTarget
        ImVec4::new(0.71, 0.78, 0.69, 0.80), // ImGuiCol_NavHighlight 
        ImVec4::new(0.70, 0.70, 0.70, 0.70), // ImGuiCol_NavWindowingHighlight 
        ImVec4::new(0.70, 0.70, 0.70, 0.30), // ImGuiCol_NavWindowingHighlight 
        ImVec4::new(0.20, 0.20, 0.20, 0.35), // ImGuiCol_ModalWindowDarkening
        ];
}

fn stream_ui(ui: &Ui, selection: &mut StreamOption, input_settings: &mut StreamSettings, imgui_str: &mut ImString) {
    let mut input_selection: i32 = *selection as i32;

    ui.columns(4, im_str!("SelectInputType"), false);
    ui.radio_button(im_str!("File"),       &mut input_selection, StreamOption::File as i32);
    ui.next_column();
    ui.radio_button(im_str!("UDP"),        &mut input_selection, StreamOption::Udp as i32);
    ui.next_column();
    ui.radio_button(im_str!("TCP Client"), &mut input_selection, StreamOption::TcpClient as i32);
    ui.next_column();
    ui.radio_button(im_str!("TCP Server"), &mut input_selection, StreamOption::TcpServer as i32);

    *selection = num::FromPrimitive::from_i32(input_selection).unwrap();

    ui.columns(1, im_str!("default"), false);
    match selection {
        StreamOption::File => {
            ui.text(im_str!("Select Input File Parameters:"));
            input_string(&ui, im_str!("File Name"), &mut input_settings.file.file_name, imgui_str);
        },

        StreamOption::Udp => {
            ui.text(im_str!("Select Udp Socket Parameters:"));
            ui.columns(2, im_str!("UdpSocketCols"), false);
            input_string(&ui, im_str!("IP Address"), &mut input_settings.udp.ip, imgui_str);
            ui.next_column();
            input_port(&ui, &mut im_str!("Port"), &mut input_settings.udp.port);
        },

        StreamOption::TcpClient => {
            ui.text(im_str!("Select Tcp Client Parameters:"));
            ui.columns(2, im_str!("UdpSocketCols"), false);
            input_string(&ui, im_str!("IP Address"), &mut input_settings.tcp_client.ip, imgui_str);
            ui.next_column();
            input_port(&ui, im_str!("Port"), &mut input_settings.tcp_client.port);
        },

        StreamOption::TcpServer => {
            ui.text(im_str!("Select Tcp Server Socket Parameters:"));
            ui.columns(2, im_str!("UdpSocketCols"), false);
            input_string(&ui, im_str!("IP Address"), &mut input_settings.tcp_server.ip, imgui_str);
            ui.next_column();
            input_port(&ui, im_str!("Port"), &mut input_settings.tcp_server.port);
        },
    }
}

fn load_config(file_name: &String) -> Option<AppConfig> {
    let result: Option<AppConfig>;

    match File::open(file_name) {
        Ok(file_opened) => {
            let mut file = file_opened;
            let mut config_str = String::new();

            file.read_to_string(&mut config_str);

            match serde_json::from_str(&config_str) {
                Ok(config) => {
                    result = Some(config);
                }

                Err(_) => {
                    result = None;
                }
            }
        },

        Err(_) => {
            result = None;
        },
    }

    result
}

fn save_config(config: &AppConfig, config_file_name: &String) {
    let mut file = File::create(&config_file_name.clone()).unwrap();
    file.write_all(&serde_json::to_string_pretty(&config).unwrap().as_bytes());
}

fn run_gui(receiver: Receiver<GuiMessage>, sender: Sender<ProcessingMsg>) {
    let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    {
        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(3, 0);
    }

    let window = video.window("CCSDS Packet Router", WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32)
        .position_centered()
        .resizable()
        .opengl()
        .allow_highdpi()
        .build()
        .unwrap();

    let _gl_context = window.gl_create_context().expect("Couldn't create GL context");
    gl::load_with(|s| video.gl_get_proc_address(s) as _);

    let mut imgui = imgui::ImGui::init();
    imgui.set_ini_filename(None);

    let mut imgui_sdl2 = imgui_sdl2::ImguiSdl2::new(&mut imgui);

    let renderer = imgui_opengl_renderer::Renderer::new(&mut imgui, |s| video.gl_get_proc_address(s) as _);

    let mut event_pump = sdl_context.event_pump().unwrap();


    // buffer for imgui strings
    let mut imgui_str = ImString::with_capacity(256);

    /* Application State */
    let mut packet_history: PacketHistory = HashMap::new();

    let mut config: AppConfig = Default::default();
    // NOTE make this an input
    let mut config_file_name = "ccsds_router.json".to_string();

    // NOTE this could be a state machine instead of bools
    let mut paused = false;
    let mut processing = false;

    let mut timestamp_selection: i32 = 1;

    // Load the initial configuration
    match load_config(&config_file_name.clone()) {
      Some(config_read) => {
          let config_used = format!("Configuration Used: {}", config_file_name);
          info!("{}", config_used);
          config = config_read;
      },

      None => {
          // use defaults if no config was read
          info!("Default Configuration Used");
          config = Default::default();
      },
    }

    match config.theme {
        GuiTheme::Dark => {
            set_style_dark(imgui.style_mut());
        },

        GuiTheme::Light => {
            set_style_light(imgui.style_mut());
        },
    }


    'running: loop {
        use sdl2::event::Event;
        use sdl2::keyboard::Keycode;

        for event in event_pump.poll_iter() {
            imgui_sdl2.handle_event(&mut imgui, &event);
            if imgui_sdl2.ignore_event(&event) { continue; }

            match event {
                Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running;
                },
                _ => {}
            }
        }

        /* Read Updates from Packet Processing Thread */
        while let Ok(msg_result) = receiver.recv_timeout(time::Duration::from_millis(0)) {

            match msg_result {
                    GuiMessage::Terminate => {
                    break 'running;
                },

                GuiMessage::PacketUpdate(packet_update) => {
                    packet_history.entry(packet_update.apid)
                        .or_default()
                        .update(packet_update);
                },

                GuiMessage::Finished => {
                    processing = false;
                },

                GuiMessage::Error(error_msg) => {
                    error!("{}", error_msg);
                },
            }
        }

        /* UI */
        let ui = imgui_sdl2.frame(&window, &mut imgui, &event_pump);

        ui.window(im_str!(""))
            .position((0.0, 0.0), ImGuiCond::FirstUseEver)
            .size((WINDOW_WIDTH, WINDOW_HEIGHT), ImGuiCond::FirstUseEver)
            .title_bar(false)
            .build(|| {
                /* Configuration Settings */
                ui.text("Configuration");
                configuration_ui(&ui, &mut config, &mut config_file_name, &mut imgui_str);

                /* Source Selection */
                ui.text("Input Settings");
                ui.child_frame(im_str!("SelectInputType"), (WINDOW_WIDTH - 15.0, 65.0))
                    .show_borders(true)
                    .build(|| {
                        stream_ui(&ui, &mut config.input_selection, &mut config.input_settings, &mut imgui_str);
                    });

                ui.text("Output Settings");
                ui.child_frame(im_str!("SelectOutputType"), (WINDOW_WIDTH - 15.0, 65.0))
                    .show_borders(true)
                    .build(|| {
                        stream_ui(&ui, &mut config.output_selection, &mut config.output_settings, &mut imgui_str);
                    });

                /* CCSDS Packet Settings */
                ui.text("CCSDS Settings");
                packet_settings_ui(&ui, &mut config, &mut timestamp_selection);

                /* Packet Statistics */
                ui.text("Packet Statistics");
                packet_statistics_ui(&ui, &packet_history);

                /* Control Buttons */
                if ui.small_button(im_str!("Clear Stats")) {
                    info!("Clearing Statistics");
                    packet_history.clear();
                }

                if paused {
                    if ui.small_button(im_str!("Continue ")) {
                        info!("Continuing Processing");
                        sender.send(ProcessingMsg::Continue).unwrap();
                        paused = false;
                        processing = true;
                    }

                    ui.same_line(0.0);

                    if ui.small_button(im_str!("Cancel")) {
                        info!("Cancelled Processing");
                        processing = false;
                        paused = false;
                        sender.send(ProcessingMsg::Cancel).unwrap();
                    }
                }
                else if processing {
                    if ui.small_button(im_str!("  Pause  ")) {
                        info!("Paused Processing");
                        processing = false;
                        paused = true;
                        sender.send(ProcessingMsg::Pause).unwrap();
                    }

                    ui.same_line(0.0);

                    if ui.small_button(im_str!("Cancel")) {
                        info!("Cancelled Processing");
                        processing = false;
                        paused = false;
                        sender.send(ProcessingMsg::Cancel).unwrap();
                    }
                }
                else {
                    // NOTE this needs to be redone to allow pausing and resetting
                    if ui.small_button(im_str!("Start")) {
                        processing = true;

                        info!("Start Processing");

                        let endianness =
                            if config.little_endian_ccsds {
                                Endianness::Little
                            } else {
                                Endianness::Big
                            };

                        sender.send(ProcessingMsg::Start(config.clone())).unwrap();
                    }
                }

                if ui.small_button(im_str!("Exit")) {
                    sender.send(ProcessingMsg::Terminate).unwrap();
                }
            });


        unsafe {
            gl::ClearColor(0.2, 0.2, 0.2, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        renderer.render(ui);

        window.gl_swap_window();


        ::std::thread::sleep(::std::time::Duration::new(0, 1_000_000_000u32 / 30));
    }

    match sender.send(ProcessingMsg::Terminate) {
        Ok(_) => {
            // NOTE awkward
            // Wait to receive terminate message from processing thread
            // should be looking for errors to log.
            while let Ok(_) = receiver.recv_timeout(time::Duration::from_millis(500)) {
            }
        }

        Err(_) => {},
    }
}

/* Gui Input Functions */
fn input_port(ui: &Ui, label: &ImStr, port: &mut u16) {
    let mut tmp = *port as i32;
    ui.input_int(label, &mut tmp).build();
    *port = tmp as u16;
}

fn configuration_ui(ui: &Ui, config: &mut AppConfig, config_file_name: &mut String, imgui_str: &mut ImString) {
    ui.child_frame(im_str!("Configuration"), (WINDOW_WIDTH - 15.0, 50.0))
        .show_borders(true)
        .build(|| {
            input_string(ui, im_str!("Configuration File"), config_file_name, imgui_str);

            if ui.small_button(im_str!("Save")) {
                save_config(config, &config_file_name.clone());
            }

            ui.same_line(0.0);

            if ui.small_button(im_str!("Load")) {
                match load_config(&config_file_name.clone()) {
                  Some(config_read) => {
                      *config = config_read;
                  },

                  None => {
                      error!("Could not load configuration file: {}", config_file_name);
                  },
                }
            }
        });
}

fn packet_settings_ui(ui: &Ui, config: &mut AppConfig, timestamp_selection: &mut i32) {
    ui.child_frame(im_str!("CcsdsSettings"), (WINDOW_WIDTH - 15.0, 200.0))
        .show_borders(true)
        .build(|| {
            ui.columns(2, im_str!("CcsdsSettings"), false);
            // Fixed or variable size packets
            let mut fixed_size_packets: bool = config.packet_size != PacketSize::Variable;
            ui.checkbox(im_str!("Fixed Size Packets"), &mut fixed_size_packets);
            if fixed_size_packets {
                ui.same_line(0.0);
                let mut packet_size: i32 = config.packet_size.num_bytes() as i32;
                ui.input_int(im_str!("Packet Size (bytes)"), &mut packet_size).build();
                config.packet_size = PacketSize::Fixed(packet_size as u16);
            }
            else {
                config.packet_size = PacketSize::Variable;
            }

            ui.next_column();

            // Endianness settings
            ui.checkbox(im_str!("Little Endian CCSDS Primary Header"), &mut config.little_endian_ccsds);
            ui.next_column();
            ui.separator();

            // Pre and post section settings
            ui.text("Prefix Bytes: ");
            ui.same_line(0.0);
            ui.input_int(im_str!(""), &mut config.prefix_bytes).build();
            ui.next_column();
            ui.checkbox(im_str!("Keep Prefix Bytes"), &mut config.keep_prefix);
            ui.next_column();

            ui.text("Postfix Bytes:");
            ui.same_line(0.0);
            ui.input_int(im_str!(""), &mut config.postfix_bytes).build();
            ui.next_column();
            ui.checkbox(im_str!("Keep Postfix Bytes"), &mut config.keep_postfix);
            ui.next_column();
            ui.separator();
            
            // Timestamp settings
            ui.text("Time Settings");
            ui.columns(4, im_str!("SelectTimestampOption"), false);
            ui.radio_button(im_str!("Forward Through"), timestamp_selection, 1);
            ui.next_column();
            ui.radio_button(im_str!("Replay"), timestamp_selection, 2);
            ui.next_column();
            ui.radio_button(im_str!("Delay"), timestamp_selection, 3);
            ui.next_column();
            ui.radio_button(im_str!("Throttle"), timestamp_selection, 4);
            ui.next_column();

            ui.columns(2, im_str!("SelectTimestampSettings"), false);
            match timestamp_selection {
                // ASAP
                1 => {
                    // no options required- just go as fast as possible
                    config.timestamp_setting = TimestampSetting::Asap;
                },

                // Replay
                2 => {
                    timestamp_def_ui(&ui, &mut config.timestamp_def);
                    config.timestamp_setting = TimestampSetting::Replay;
                },

                // Delay
                3 => {
                    timestamp_def_ui(&ui, &mut config.timestamp_def);
                    match config.timestamp_setting {
                        TimestampSetting::Delay(delay) => {
                            ui.next_column();
                            ui.text("Delay Time");
                            ui.next_column();
                            let mut delay_time = delay.as_fractional_secs() as f32;
                            ui.input_float(im_str!(""), &mut delay_time).build();
                            config.timestamp_setting =
                                TimestampSetting::Delay(Duration::new(delay_time as u64,
                                                                      (delay_time.fract() * 1000000000.0) as u32));
                        }

                        _ => {
                            config.timestamp_setting = TimestampSetting::Delay(Duration::new(0, 0));
                        }
                    }
                },

                // Throttle
                4 => {
                    timestamp_def_ui(&ui, &mut config.timestamp_def);
                    match config.timestamp_setting {
                        TimestampSetting::Throttle(delay) => {
                            ui.next_column();
                            ui.text("Max Time");
                            ui.next_column();
                            let mut delay_time = delay.as_fractional_secs() as f32;
                            ui.input_float(im_str!(""), &mut delay_time).build();
                            config.timestamp_setting =
                                TimestampSetting::Throttle(Duration::new(delay_time as u64,
                                                                         (delay_time.fract() * 1000000000.0) as u32));
                        }

                        _ => {
                            config.timestamp_setting = TimestampSetting::Throttle(Duration::new(0, 0));
                        }
                    }
                },

                _ => unreachable!(),

            }
        });
}

fn packet_statistics_ui(ui: &Ui, packet_history: &PacketHistory) {
    ui.child_frame(im_str!("Apid Statistics"), (WINDOW_WIDTH - 15.0, 220.0))
        .show_borders(true)
        .show_scrollbar(true)
        .always_show_vertical_scroll_bar(true)
        .build(|| {
            let count = packet_history.len() as i32;
            let mut string = String::from("Apids Seen: ");
            string.push_str(&count.to_string());
            ui.text(string.as_str());

            ui.separator();

            ui.columns(6, im_str!("PacketStats"), true);

            for packet_stats in packet_history.values() {
                ui.text("Apid: ");

                ui.next_column();
                ui.text(format!("{:>5}", &packet_stats.apid.to_string()));

                ui.next_column();
                ui.text("Count: ");

                ui.next_column();
                ui.text(format!("{:>5}", packet_stats.packet_count.to_string()));

                ui.next_column();
                ui.text("Bytes: ");

                ui.next_column();
                ui.text(format!("{:>9}", &packet_stats.byte_count.to_string()));

                ui.next_column();
            }

            if packet_history.len() > 0 {
                ui.separator();

                ui.text("Total Apids:");

                ui.next_column();
                ui.text(packet_history.len().to_string());

                ui.next_column();
                ui.text("Total Packets");

                ui.next_column();
                let total_count = packet_history.values().map(|stats: &PacketStats| stats.packet_count as u32).sum::<u32>();
                ui.text(format!("{:>5}", total_count));

                ui.next_column();
                ui.text("Total Bytes:");

                ui.next_column();
                let total_byte_count = packet_history.values().map(|stats: &PacketStats| stats.byte_count).sum::<u32>();
                ui.text(format!("{:>9}", total_byte_count));

                ui.next_column();
            }
        });
}

fn timestamp_def_ui(ui: &Ui, timestamp_def: &mut TimestampDef) {
    ui.text("Byte Offset After Primary Header");
    ui.next_column();
    ui.input_int(im_str!(""), &mut timestamp_def.offset).build();

    ui.next_column();
    ui.text("Number of Bytes in Seconds");
    ui.next_column();
    ui.input_int(im_str!(""), &mut timestamp_def.num_bytes_seconds).build();

    ui.next_column();
    ui.text("Number of Bytes in Subseconds");
    ui.next_column();
    ui.input_int(im_str!(""), &mut timestamp_def.num_bytes_subseconds).build();

    ui.next_column();
    ui.text("Subsecond Resolution");
    ui.next_column();
    ui.input_float(im_str!(""), &mut timestamp_def.subsecond_resolution).build();
}

fn input_string(ui: &Ui, label: &ImStr, string: &mut String, imgui_str: &mut ImString) {
    imgui_str.clear();
    imgui_str.push_str(&string);
    ui.input_text(label, imgui_str).build();
    string.clear();
    string.push_str(&imgui_str.to_str());
}

