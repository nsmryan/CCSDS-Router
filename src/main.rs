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

extern crate sdl2;
extern crate imgui;
extern crate imgui_sdl2;
extern crate gl;
extern crate imgui_opengl_renderer;


use std::time;
use std::thread;
use std::io::{Write, Read};
use std::default::Default;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::fs::File;
use std::fs::create_dir;

use ccsds_primary_header::*;

use byteorder::{LittleEndian};

use simplelog::*;

use chrono::prelude::*;

use imgui::*;

mod stream;
use stream::*;


const WINDOW_WIDTH:  f32 = 640.0;
const WINDOW_HEIGHT: f32 = 640.0;


type Apid = u16;

/* Application Configuration */
#[derive(Default, Debug, Serialize, Deserialize)]
struct AppConfig {
    read_config:  StreamSettings,
    read_option:  StreamOption,
    write_config: StreamSettings,
    write_option: StreamOption,
}

/* Packet Data */
type PacketHistory = HashMap<Apid, PacketStats>;

#[derive(Default, Debug)]
struct PacketStats {
    apid: Apid,
    packet_count: u16,
    byte_count: u32,
}

#[derive(Debug)]
struct PacketUpdate {
    apid: Apid,
    packet_length: u16,
}

impl PacketStats {
    fn update(&mut self, packet_update: PacketUpdate) {
        self.apid = packet_update.apid;
        self.packet_count += 1;
        self.byte_count += packet_update.packet_length as u32;
    }
}

/* Messages Generated During Packet Processing */
#[derive(Debug)]
enum GuiMessage {
    PacketUpdate(PacketUpdate),
    Finished(),
    Terminate(),
    Error(String),
}

#[derive(Debug)]
enum ProcessingMsg {
    Start(StreamSettings, StreamOption, StreamSettings, StreamOption),
    Stop(),
    Cancel(),
    Terminate(),
}


fn main() {
    // Set Up Logging
    create_dir("log");
    let date = Local::now();
    let log_name = format!("{}", date.format("log/ccsds_router_log_%Y%m%d_%H_%M_%S.log"));
    let logger = WriteLogger::init(LevelFilter::max(), Config::default(), File::create(log_name).unwrap());


    // Spawn processing thread
    let (gui_sender,  gui_receiver)  = channel::<GuiMessage>();
    let (proc_sender, proc_receiver) = channel::<ProcessingMsg>();

    let ccsds_thread = thread::spawn(move || {
        stream_ccsds( gui_sender, proc_receiver );
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
            input_string(&ui, im_str!("IP Address"), &mut input_settings.udp.ip, imgui_str);
            input_port(&ui, &mut im_str!("Port"), &mut input_settings.udp.port);
        },

        StreamOption::TcpClient => {
            ui.text(im_str!("Select Tcp Client Parameters:"));
            input_string(&ui, im_str!("IP Address"), &mut input_settings.tcp_client.ip, imgui_str);
            input_port(&ui, im_str!("Port"), &mut input_settings.tcp_client.port);
        },

        StreamOption::TcpServer => {
            ui.text(im_str!("Select Tcp Server Socket Parameters:"));
            input_string(&ui, im_str!("IP Address"), &mut input_settings.tcp_server.ip, imgui_str);
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

    set_style_dark(imgui.style_mut());
    //set_style_light(imgui.style_mut());

    let mut imgui_sdl2 = imgui_sdl2::ImguiSdl2::new(&mut imgui);

    let renderer = imgui_opengl_renderer::Renderer::new(&mut imgui, |s| video.gl_get_proc_address(s) as _);

    let mut event_pump = sdl_context.event_pump().unwrap();


    // buffer for imgui strings
    let mut imgui_str = ImString::with_capacity(256);

    /* Application State */
    let mut packet_history: PacketHistory = HashMap::new();

    let mut config: AppConfig = Default::default();
    let mut config_file_name = "ccsds_router.json".to_string();

    let mut processing = 0;

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
                    GuiMessage::Terminate() => {
                    break 'running;
                },

                GuiMessage::PacketUpdate(packet_update) => {
                    packet_history.entry(packet_update.apid)
                        .or_default()
                        .update(packet_update);
                },

                GuiMessage::Finished() => {
                    processing = 0;
                },

                GuiMessage::Error(error_msg) => {
                    error!("{}", error_msg);
                },
            }
        }

        let ui = imgui_sdl2.frame(&window, &mut imgui, &event_pump);

        ui.window(im_str!(""))
            .position((0.0, 0.0), ImGuiCond::FirstUseEver)
            .size((WINDOW_WIDTH, WINDOW_HEIGHT), ImGuiCond::FirstUseEver)
            .title_bar(false)
            .build(|| {
                /* Configuration Settings */
                ui.text("Configuration");
                ui.child_frame(im_str!("Configuration"), (WINDOW_WIDTH - 15.0, 50.0))
                    .show_borders(true)
                    .build(|| {
                        input_string(&ui, im_str!("Configuration File"), &mut config_file_name, &mut imgui_str);

                        if ui.small_button(im_str!("Save")) {
                            save_config(&config, &config_file_name.clone());
                        }

                        ui.same_line(0.0);

                        if ui.small_button(im_str!("Load")) {
                            match load_config(&config_file_name.clone()) {
                              Some(config_read) => {
                                  config = config_read;
                              },

                              None => {
                                  // TODO Report error in error log
                              },
                            }
                        }
                    });

                /* Source Selection */
                ui.text("Input Settings");
                ui.child_frame(im_str!("SelectInputType"), (WINDOW_WIDTH - 15.0, 100.0))
                    .show_borders(true)
                    .build(|| {
                        stream_ui(&ui, &mut config.read_option, &mut config.read_config, &mut imgui_str);
                    });

                ui.text("Output Settings");
                ui.child_frame(im_str!("SelectOutputType"), (WINDOW_WIDTH - 15.0, 100.0))
                    .show_borders(true)
                    .build(|| {
                        stream_ui(&ui, &mut config.write_option, &mut config.write_config, &mut imgui_str);
                    });

                /* Packet Statistics */
                ui.text("Packet Statistics");
                ui.child_frame(im_str!("Apid Statistics"), (WINDOW_WIDTH - 15.0, 250.0))
                    .show_borders(true)
                    .build(|| {
                        let count = packet_history.len() as i32;
                        let mut string = String::from("Apids Seen: ");
                        string.push_str(&count.to_string());
                        ui.text(string.as_str());

                        ui.separator();

                        ui.columns(6, im_str!("PacketStats"), true);

                        for packet_stats in packet_history.values() {
                            let mut string = String::from("Apid: ");
                            ui.text(string.as_str());

                            ui.next_column();
                            string = format!("{:>5}", &packet_stats.apid.to_string());
                            ui.text(string.as_str());

                            ui.next_column();
                            let mut string = String::from("Count: ");
                            ui.text(string.as_str());

                            ui.next_column();
                            string = format!("{:>5}", packet_stats.packet_count.to_string());
                            ui.text(string.as_str());

                            ui.next_column();
                            let mut string = String::from("Bytes: ");
                            ui.text(string.as_str());

                            ui.next_column();
                            string = format!("{:>9}", &packet_stats.byte_count.to_string());
                            ui.text(string.as_str());

                            ui.next_column();
                        }
                    });

                if processing == 0 {
                    if ui.small_button(im_str!("Start")) {
                        processing = 1;
                        sender.send(ProcessingMsg::Start(config.read_config.clone(),
                                                         config.read_option.clone(),
                                                         config.write_config.clone(),
                                                         config.write_option.clone())).unwrap();
                    }
                }
                else {
                    if ui.small_button(im_str!("Stop ")) {
                        processing = 0;
                        sender.send(ProcessingMsg::Stop()).unwrap();
                    }
                }

                ui.same_line(0.0);

                if ui.small_button(im_str!("Clear Stats")) {
                    packet_history.clear();
                }

                ui.same_line(0.0);

                if ui.small_button(im_str!("Exit")) {
                    sender.send(ProcessingMsg::Terminate()).unwrap();
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

    sender.send(ProcessingMsg::Terminate()).unwrap();
}

/* Gui Input Functions */
fn input_port(ui: &Ui, label: &ImStr, port: &mut u16) {
    let mut tmp = *port as i32;
    ui.input_int(label, &mut tmp).build();
    *port = tmp as u16;
}

fn input_string(ui: &Ui, label: &ImStr, string: &mut String, imgui_str: &mut ImString) {
    imgui_str.clear();
    imgui_str.push_str(&string);
    ui.input_text(label, imgui_str).build();
    string.clear();
    string.push_str(&imgui_str.to_str());
}


/* Packet Processing Thread */
fn stream_ccsds(sender: Sender<GuiMessage>, receiver: Receiver<ProcessingMsg>) {
    let mut packet: Vec<u8> = Vec::with_capacity(4096);

    let mut processing = false;
    let mut terminating = false;

    let mut input_settings:  StreamSettings;
    let mut output_settings: StreamSettings;

    let mut in_stream  = ReadStream::Null();
    let mut out_stream = WriteStream::Null();

    while !terminating {
        while !processing {
            let proc_msg = receiver.recv().ok();

            match proc_msg {
                Some(ProcessingMsg::Start(new_input_settings, input_option, new_output_settings, output_option)) => {
                    input_settings  = new_input_settings;
                    output_settings = new_output_settings;

                    match open_read_stream(&input_settings, input_option) {
                      Some(stream) => {
                          in_stream  = stream;

                          match open_write_stream(&output_settings, output_option) {
                            Some(stream) => {
                                out_stream = stream;
                                processing = true;
                            },

                            None => {
                                sender.send(GuiMessage::Error("Could not open for writing".to_string())).unwrap();
                            },
                          }
                      },

                      None => {
                          sender.send(GuiMessage::Error("Could not open for reading".to_string())).unwrap();
                      },
                    }

                    if !processing {
                        // setting these to another variant drops the previous value,
                        // closing whatever stream was contained.
                        in_stream  = ReadStream::Null();
                        out_stream = WriteStream::Null();
                    }

                },

                Some(ProcessingMsg::Terminate()) => {
                    terminating = true;
                    break;
                },

                _ => {},
            }
        }

        if terminating {
            break;
        }

        // NOTE this is an infinite loop. for files we will have to detect the
        // end and break the loop.
        loop {
            /* Check for Control Messages */
            let proc_msg = receiver.recv_timeout(time::Duration::from_millis(0)).ok();

            match proc_msg {
                Some(ProcessingMsg::Stop()) => {
                    processing = false;
                    continue;
                },

                Some(ProcessingMsg::Cancel()) => {
                    processing = false;
                    // TODO should close streams
                    continue;
                },

                Some(ProcessingMsg::Terminate()) => {
                    terminating = true;
                    break;
                },

                _ => {},
            }

            /* Process a Packet */
            stream_read_packet(&mut in_stream, &mut packet);

            stream_send(&mut out_stream, &packet);

            /* Report packet to GUI */
            // NOTE this is an awkward way to read out a header from a Vec<u8>
            // could have a packet struct of vec<u8> and header
            let mut header: [u8;6] = [0; 6];
            for byte_index in 0..6 {
                header[byte_index] = packet[byte_index];
            }
            let pri_header = PrimaryHeader::<LittleEndian>::new(header);


            let packet_update = PacketUpdate { apid: pri_header.control.apid(),
                                               packet_length: packet.len() as u16,
                                             };

            sender.send(GuiMessage::PacketUpdate(packet_update)).unwrap();
        }

        sender.send(GuiMessage::Terminate()).unwrap();
    }
}
