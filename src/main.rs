//! CCSDS Router is a GUI program for forwarding CCSDS packets from
//! an input source (file, UDP socket, TCP socket) to an output (file,
//! UDP socket, TCP socket).
//!
//! The intended use cases are:
//! * Replaying stored packets, such as for testing.
//! * Forwarding packets from one port to another as a translation between
//!     system interfaces.
//! * Delay packets to simulate communication delay for testing.
//!
//!
//! # Configuration
//! The GUI options are all in a configuration file in JSON format.
//!
//! If the configuration file is not provided, or it cannot be read, the default
//! configuration will be used and a warning will be logged.
//!
//! The configuration file used is reported in the application log as a record of
//! which configuration was used.
//!
//!
//! # Logging
//! This application creates logs in a directory called 'logs'. Each log is timestamped
//! with the time that the application was started.
//!
//! The logs are mostly used to indicate errors. These messages are also output to the console.
//!
//!
//! # Packet Forward Timing
//! This program provides several options for when to forward a packet from
//! the input to the output. The packets can be forwarded as soon as they
//! are read, they can be delayed by a fixed amount, the packet rate can be throttled to only allow
//! packets at a certain rate, or the packets can be replayed using the timestamps within the
//! packets themselves.
//! For replaying packets, the time stamp must be in the form of a number of seconds followed by
//! subseconds, each of which must be 1/2/or 4 bytes. The subseconds have a configurable
//! resolution.
//! This covers many common cases, especially those that follow the CCSDS time format standard.
//!
//!
//! # Framing
//! The CCSDS packets processsed by this application can have a header or footer from another
//! protocol. These must be fixed size, and can be forwarded along with the CCSDS packets or
//! omitted.
//!
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

#[macro_use] extern crate structopt;

extern crate hexdump;

extern crate ctrlc;

extern crate sdl2;
extern crate imgui;
extern crate imgui_sdl2;
extern crate gl;
extern crate imgui_opengl_renderer;


use std::time::{Duration, SystemTime};
use std::thread;
use std::io::{Write, Read};
use std::default::Default;
use std::collections::{VecDeque};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::fs::File;
use std::fs::create_dir;
use std::path::PathBuf;
use std::cmp::{min, max};

use simplelog::*;

use chrono::prelude::*;

use floating_duration::TimeAsFloat;

use structopt::*;

use hexdump::*;

use imgui::*;

mod stream;
use stream::*;

mod types;
use types::*;

mod processing;
use processing::*;

mod style;
use style::*;


/// Window width given to SDL
const WINDOW_WIDTH:  f32 = 680.0;

/// Window height given to SDL
const WINDOW_HEIGHT: f32 = 740.0;

const STATS_FRAME_HEIGHT: f32 = 170.0;

const CONFIG_SETTINGS_FRAME_HEIGHT: f32 = 50.0;

const INPUT_SETTINGS_FRAME_HEIGHT: f32 = 100.0;

const OUTPUT_SETTINGS_FRAME_HEIGHT: f32 = 100.0;

const CCSDS_SETTINGS_FRAME_HEIGHT: f32 = 180.0;

const LOG_DIRECTORY: &str = "logs";


#[derive(Debug, StructOpt)]
#[structopt(name = "ccsds_router", about = "CCSDS Router moves CCSDS packets from an input to an output")]
struct Opt {
    #[structopt(short = "s", long = "supressgui")]
    supress_gui: bool,

    #[structopt(parse(from_os_str))]
    config_file_name: Option<PathBuf>,
}

fn main() {
    let opt = Opt::from_args();

    let mut config: AppConfig;

    let mut config_file_name: String;

    // Set Up Logging
    // we ignore the result as it will fail if the directory already exists.
    let _ = create_dir(LOG_DIRECTORY);

    let date = Local::now();
    let log_name = format!("{}/{}", LOG_DIRECTORY, date.format("ccsds_router_log_%Y%m%d_%H_%M_%S.log"));
    let _ = CombinedLogger::init(vec!(TermLogger::new(LevelFilter::max(),   Config::default()).unwrap(),
                                      WriteLogger::new(LevelFilter::max(), Config::default(), File::create(log_name).unwrap())
                                      )).unwrap();


    // Read configuration file
    match opt.config_file_name {
        Some(path) => config_file_name = path.to_string_lossy().to_string(),
        None => config_file_name = "ccsds_router.json".to_string(),
    }

    // Load the initial configuration
    match load_config(&config_file_name) {
      Some(config_read) => {
          let config_used = format!("Configuration Used: {}", config_file_name);
          info!("{}", config_used);

          config = config_read;
      },

      None => {
          // use defaults if no config was read
          warn!("Configuration '{}' provided. Default Configuration Used", config_file_name);
          config = Default::default();

          // the default max length is 0xFFFF in the length field, 
          // plus the size of a CCSDS Primary header, plus 1.
          config.max_length_bytes = 65535 + 6 + 1;
      },
    }

    // make sure there is at least one of the output settings
    if config.output_settings.len() == 0 {
        config.output_settings = vec!(Default::default());
    }
    if config.output_selection.len() == 0 {
        config.output_selection = vec!(Default::default());
    }
    if config.allowed_output_apids.len() == 0 {
        config.allowed_output_apids = vec!(None);
    }

    // Spawn processing thread
    let (gui_sender,  gui_receiver)  = channel::<GuiMessage>();
    let (proc_sender, proc_receiver) = channel::<ProcessingMsg>();

    let ccsds_thread = thread::spawn(move || {
        process_thread( gui_sender, proc_receiver );
    });

    // Set up ctrl-c handling
    let proc_sender_clone = proc_sender.clone();
    ctrlc::set_handler(move || {
        proc_sender_clone.send(ProcessingMsg::Terminate).unwrap();
        std::thread::sleep(Duration::from_millis(200));
    }).expect("Error setting up ctrl-c handling");

    // if we run without a GUI, make sure to autostart or nothing will happen.
    if opt.supress_gui {
        config.auto_start = true;
    }

    // If auto start is selected, start the processing thread immediately
    if config.auto_start {
        info!("Auto Start Processing. Configuration file {}", config_file_name);

        proc_sender.send(ProcessingMsg::Start(config.clone())).unwrap();
    }

    if opt.supress_gui {
        info!("Running without GUI");
        // if no gui is run, just read messages until the processing thread is finished
        while let Ok(msg_result) = gui_receiver.recv_timeout(Duration::from_millis(500)) {

            match msg_result {
                    GuiMessage::Terminate => {
                    break;
                },

                GuiMessage::PacketUpdate(packet_update) => {
                },

                GuiMessage::PacketDropped(header) => {
                },

                GuiMessage::Finished => {
                    break;
                },

                GuiMessage::Error(error_msg) => {
                    error!("{}", error_msg);
                },
            }
        }
    } else {
        // Run GUI main loop
        run_gui( &mut config, &mut config_file_name, gui_receiver, proc_sender );
    }


    // Clean up and Exit 
    ccsds_thread.join().unwrap();


    info!("Exiting");
}

fn run_gui(config: &mut AppConfig, config_file_name: &mut String, receiver: Receiver<GuiMessage>, sender: Sender<ProcessingMsg>) {
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
    let mut processing_stats: ProcessingStats = Default::default();

    // NOTE this could be a state machine instead of bools
    let mut paused = false;
    let mut processing = config.auto_start;

    let mut input_settings_shown = true;
    let mut output_settings_shown = true;
    let mut ccsds_settings_shown = true;
    let mut config_settings_shown = true;

    let mut packets_dropped = 0;

    let mut output_index = 0;

    // index of selection for how to treat timestamps
    let mut timestamp_selection: i32 = 1;

    let mut packet_recv_diffs: VecDeque<SystemTime> = VecDeque::new();
    let mut packet_recv_bytes: usize = 0;

    match config.theme {
        GuiTheme::Dark => {
            set_style_dark(imgui.style_mut());
        },

        GuiTheme::Light => {
            set_style_light(imgui.style_mut());
        },
    }


    // Main GUI event loop
    'running: loop {
        /* SDL Events */
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
        while let Ok(msg_result) = receiver.recv_timeout(Duration::from_millis(0)) {

            match msg_result {
                    GuiMessage::Terminate => {
                    break 'running;
                },

                GuiMessage::PacketUpdate(packet_update) => {
                    let apid = packet_update.apid;
                    let packet_stats = processing_stats.packet_history.entry(apid).or_default();
                    let packet_length = packet_update.packet_length as usize;
                    packet_stats.update(packet_update);
                    packet_recv_diffs.push_back(packet_stats.recv_time);
                    packet_recv_bytes += packet_length;
                },

                GuiMessage::PacketDropped(header) => {
                    packets_dropped += 1;
                },

                GuiMessage::Finished => {
                    processing = false;
                },

                GuiMessage::Error(error_msg) => {
                    error!("{}", error_msg);
                },
            }
        }

        if packet_recv_diffs.len() > 0  &&
              SystemTime::now().duration_since(*packet_recv_diffs.get(0).unwrap()).unwrap() > Duration::from_secs(1) {
            processing_stats.packets_per_second = packet_recv_diffs.len();
            processing_stats.bytes_per_second = packet_recv_bytes;
            packet_recv_diffs.clear();
            packet_recv_bytes = 0;
        }

        /* IMGUI UI */
        let ui = imgui_sdl2.frame(&window, &mut imgui, &event_pump);

        ui.window(im_str!(""))
            .position((0.0, 0.0), ImGuiCond::FirstUseEver)
            .size((WINDOW_WIDTH, WINDOW_HEIGHT), ImGuiCond::FirstUseEver)
            .title_bar(false)
            .movable(false)
            .scrollable(false)
            .resizable(false)
            .collapsible(false)
            .build(|| {
                /* Configuration Settings */
                ui.text("Configuration");
                ui.same_line(0.0);
                ui.with_id("ToggleConfigSettings", || {
                    // align the word 'Toggle' with other settings
                    ui.text("  ");
                    ui.same_line(0.0);
                    // button to show or hide section
                    if ui.small_button(im_str!("Toggle")) {
                        config_settings_shown = !config_settings_shown;
                    }
                });
                if config_settings_shown {
                    configuration_ui(&ui, config, config_file_name, &mut imgui_str);
                }

                /* Source Selection */
                ui.text("Input Settings");
                ui.same_line(0.0);
                ui.with_id("ToggleInputSettings", || {
                    // align the word 'Toggle' with other settings
                    ui.text(" ");
                    ui.same_line(0.0);
                    // button to show or hide section
                    if ui.small_button(im_str!("Toggle")) {
                        input_settings_shown = !input_settings_shown;
                    }
                });
                if input_settings_shown {
                    ui.child_frame(im_str!("SelectInputType"), ((WINDOW_WIDTH - 15.0), INPUT_SETTINGS_FRAME_HEIGHT))
                        .show_borders(true)
                        .collapsible(true)
                        .build(|| {
                            input_stream_ui(&ui,
                                            &mut config.input_selection,
                                            &mut config.input_settings,
                                            &mut config.allowed_input_apids,
                                            &mut imgui_str);
                        });
                }

                ui.text("Output Settings");
                ui.same_line(0.0);
                ui.with_id("ToggleOutputSettings", || {
                    // align the word 'Toggle' with other settings
                    ui.text("");
                    ui.same_line(0.0);
                    // button to show or hide section
                    if ui.small_button(im_str!("Toggle")) {
                        output_settings_shown = !output_settings_shown;
                    }
                });
                ui.same_line(0.0);
                if ui.small_button(im_str!("New")) {
                    config.output_selection.push(Default::default());
                    config.output_settings.push(Default::default());
                    config.allowed_output_apids.push(None);
                    output_index += 1;
                }
                ui.same_line(0.0);
                if ui.small_button(im_str!("Prev")) {
                    if output_index > 0 {
                        output_index -= 1;
                    }
                }
                ui.same_line(0.0);
                ui.text(format!("{}", output_index));
                ui.same_line(0.0);
                if ui.small_button(im_str!("Next")) {
                    output_index = min(output_index + 1, config.output_selection.len() - 1);
                }
                ui.same_line(0.0);
                if ui.small_button(im_str!("Delete")) {
                    // only allow deletion if this is not the last output
                    if config.output_selection.len() > 1 {
                        config.output_selection.remove(output_index);
                        config.output_settings.remove(output_index);
                        config.allowed_output_apids.remove(output_index);
                        output_index = min(output_index, config.output_selection.len() - 1);
                    }
                }
                ui.same_line(0.0);
                ui.text(format!("({})", config.output_selection.len()));
                if output_settings_shown {
                    ui.child_frame(im_str!("SelectOutputType"), ((WINDOW_WIDTH - 15.0), OUTPUT_SETTINGS_FRAME_HEIGHT))
                        .movable(true)
                        .show_borders(true)
                        .collapsible(true)
                        .show_scrollbar(true)
                        .always_show_vertical_scroll_bar(true)
                        .build(|| {
                            output_stream_ui(&ui,
                                             &mut config.output_selection[output_index],
                                             &mut config.output_settings[output_index],
                                             &mut config.allowed_output_apids[output_index],
                                             &mut imgui_str);
                        });
                }

                /* CCSDS Packet Settings */
                ui.text("CCSDS Settings");
                ui.same_line(0.0);
                ui.with_id("ToggleCcsdsSettings", || {
                    // align the word 'Toggle' with other settings
                    ui.text(" ");
                    ui.same_line(0.0);
                    // button to show or hide section
                    if ui.small_button(im_str!("Toggle")) {
                        ccsds_settings_shown = !ccsds_settings_shown;
                    }
                });
                if ccsds_settings_shown {
                    packet_settings_ui(&ui, config, &mut timestamp_selection);
                }

                /* Packet Statistics */
                ui.text("Packet Statistics");
                let mut dims = ImVec2::new(WINDOW_WIDTH - 15.0, STATS_FRAME_HEIGHT);
                if !config_settings_shown {
                    dims.y += CONFIG_SETTINGS_FRAME_HEIGHT;
                    dims.y += 2.0;
                }
                if !input_settings_shown {
                    dims.y += INPUT_SETTINGS_FRAME_HEIGHT;
                    dims.y += 2.0;
                }
                if !output_settings_shown {
                    dims.y += OUTPUT_SETTINGS_FRAME_HEIGHT;
                    dims.y += 2.0;
                }
                if !ccsds_settings_shown {
                    dims.y += CCSDS_SETTINGS_FRAME_HEIGHT;
                    dims.y += 2.0;
                }
                packet_statistics_ui(&ui, &processing_stats, packets_dropped, dims);

                /* Control Buttons */
                if ui.small_button(im_str!("Clear Stats")) {
                    info!("Clearing Statistics");
                    processing_stats = Default::default();
                }

                ui.same_line(0.0);

                if input_settings_shown && output_settings_shown && ccsds_settings_shown && config_settings_shown {
                    if ui.small_button(im_str!("Collapse All")) {
                      input_settings_shown  = false;
                      output_settings_shown = false;
                      ccsds_settings_shown  = false;
                      config_settings_shown = false;
                    }
                } else {
                    if ui.small_button(im_str!(" Expand All ")) {
                      input_settings_shown  = true;
                      output_settings_shown = true;
                      ccsds_settings_shown  = true;
                      config_settings_shown = true;
                    }
                }

                // if we are paused, ask to continue or cancel
                if paused {
                    if ui.small_button(im_str!("Continue ")) {
                        info!("Continuing Processing");
                        sender.send(ProcessingMsg::Continue).unwrap();
                        processing = true;
                        paused = false;
                    }

                    ui.same_line(0.0);

                    if ui.small_button(im_str!("Cancel")) {
                        info!("Cancelled Processing");
                        processing = false;
                        paused = false;
                        sender.send(ProcessingMsg::Cancel).unwrap();
                    }
                }
                // if we are processing packets, ask to pause
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
                // otherwise, ask if we want to start processing packets
                else {
                    if ui.small_button(im_str!("Start")) {
                        processing = true;

                        // the current configuration is always saved when processing.
                        // This is to prevent running a configuration that is not saved anywhere.
                        save_config(config, &config_file_name.clone());
                        info!("Start Processing. Configuration file {}", config_file_name);

                        sender.send(ProcessingMsg::Start(config.clone())).unwrap();
                    }
                }

                // don't exit unless the user confirms their action
                if ui.small_button(im_str!("Exit")) {
                    ui.open_popup(im_str!("Exit?"));
                }
                ui.popup_modal(im_str!("Exit?")).build(|| {
                    ui.text("Exit the application?");
                    if ui.small_button(im_str!("Exit")) {
                        sender.send(ProcessingMsg::Terminate).unwrap();
                    }

                    ui.same_line(0.0);

                    if ui.small_button(im_str!("Don't Exit")) {
                        ui.close_current_popup();
                    }
                });
            });


        unsafe {
            gl::ClearColor(0.2, 0.2, 0.2, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        renderer.render(ui);

        window.gl_swap_window();


        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
    }

    match sender.send(ProcessingMsg::Terminate) {
        Ok(_) => {
            // NOTE awkward
            while let Ok(msg) = receiver.recv_timeout(Duration::from_millis(500)) {
                match msg {
                    GuiMessage::Error(error_msg) => {
                        error!("{}", error_msg);
                    },

                    _ => {}, // ignore other errors
                }
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
    ui.child_frame(im_str!("Configuration"), (WINDOW_WIDTH - 15.0, CONFIG_SETTINGS_FRAME_HEIGHT))
      .show_borders(true)
      .collapsible(true)
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
    ui.child_frame(im_str!("CcsdsSettingsFrame"), (WINDOW_WIDTH - 15.0, CCSDS_SETTINGS_FRAME_HEIGHT))
      .collapsible(true)
      .show_borders(true)
      .build(|| {
          ui.columns(2, im_str!("CcsdsSettingsCol"), false);
          // Fixed or variable size packets
          let mut fixed_size_packets: bool = config.packet_size != PacketSize::Variable;
          ui.checkbox(im_str!("Fixed Size Packets"), &mut fixed_size_packets);
          if fixed_size_packets {
              ui.same_line(0.0);
              let mut packet_size: i32 = config.packet_size.num_bytes() as i32;
              ui.input_int(im_str!("Packet Size (bytes)"), &mut packet_size).build();
              config.packet_size = PacketSize::Fixed(packet_size as u32);
          }
          else {
              config.packet_size = PacketSize::Variable;
          }

          ui.next_column();

          // Endianness settings
          ui.checkbox(im_str!("Little Endian CCSDS Primary Header"), &mut config.little_endian_ccsds);
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Decode CCSDS Primary Header as Little Endian"));
          }
          ui.next_column();
          ui.separator();

          // Pre and post section settings
          ui.text("Header Bytes: ");
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Size of frame header in front of each CCSDS Primary Header"));
          }
          ui.same_line(0.0);
          ui.input_int(im_str!(""), &mut config.frame_settings.prefix_bytes).build();
          ui.next_column();
          ui.checkbox(im_str!("Keep Header Bytes"), &mut config.frame_settings.keep_prefix);
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Keep frame header when forwarding packet to output"));
          }
          ui.next_column();

          ui.text("Footer Bytes: ");
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Size of frame footer in front of each CCSDS Primary Header"));
          }
          ui.same_line(0.0);
          ui.input_int(im_str!(""), &mut config.frame_settings.postfix_bytes).build();
          ui.next_column();
          ui.checkbox(im_str!("Keep Footer Bytes"), &mut config.frame_settings.keep_postfix);
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Keep frame footer when forwarding packet to output"));
          }
          ui.next_column();

          ui.columns(1, im_str!("Maximum Packet Size Section"), false);
          ui.input_int(im_str!("Maximum Packet Size (Bytes)"), &mut config.max_length_bytes).build();
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Maximum packet size, ignoring frame header/footer, that will be forwarded to output"));
          }
          ui.separator();
          
          // Timestamp settings
          ui.text("Time Settings");
          ui.columns(4, im_str!("SelectTimestampOption"), false);
          ui.radio_button(im_str!("Forward Through"), timestamp_selection, 1);
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Process packets as fast as possible"));
          }
          ui.next_column();
          ui.radio_button(im_str!("Replay"), timestamp_selection, 2);
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Process packets according to their timestamps"));
          }
          ui.next_column();
          ui.radio_button(im_str!("Delay"), timestamp_selection, 3);
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Delay packets by a fixed amount"));
          }
          ui.next_column();
          ui.radio_button(im_str!("Throttle"), timestamp_selection, 4);
          if ui.is_item_hovered() {
              ui.tooltip_text(im_str!("Provide a minimum time between sending each packet"));
          }
          ui.next_column();

          //ui.columns(2, im_str!("SelectTimestampSettings"), false);
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
                  match config.timestamp_setting {
                      TimestampSetting::Delay(delay) => {
                          ui.columns(2, im_str!("SpecificTimeSettings"), false);
                          //ui.text("Delay Time");
                          //ui.next_column();
                          let mut delay_time = delay.as_fractional_secs() as f32;
                          ui.input_float(im_str!("Delay Time"), &mut delay_time).build();
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
                  match config.timestamp_setting {
                      TimestampSetting::Throttle(delay) => {
                          ui.columns(2, im_str!("SpecificTimeSettings"), false);
                          //ui.text("Time Between Packets");
                          //ui.next_column();
                          let mut delay_time = delay.as_fractional_secs() as f32;
                          ui.input_float(im_str!("Time Between Packets"), &mut delay_time).build();
                          config.timestamp_setting =
                              TimestampSetting::Throttle(Duration::new(delay_time as u64,
                                                                       (delay_time.fract() * 1_000_000_000.0) as u32));
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

fn packet_summary_ui(ui: &Ui, packet_stats: &PacketStats) {
    if ui.is_item_hovered() {
        ui.tooltip(|| {
            ui.text(format!("APID {} Hex Dump:", packet_stats.apid));
            hexdump_iter(&packet_stats.bytes).for_each(|s| ui.text(format!("{}", s)));
        });
    }
}

fn packet_statistics_ui(ui: &Ui, processing_stats: &ProcessingStats, packets_dropped: usize, dims: ImVec2) {
    ui.child_frame(im_str!("Apid Statistics"), dims)
        .show_borders(true)
        .collapsible(true)
        .show_scrollbar(true)
        .always_show_vertical_scroll_bar(true)
        .movable(true)
        .build(|| {
            let count = processing_stats.packet_history.len() as i32;
            ui.text(format!("Apids Seen: {:3} ", count));

            ui.same_line(0.0);
            ui.text(format!("Packets Dropped: {:>4}", packets_dropped));

            ui.same_line(0.0);
            ui.text(format!("Packets Per Second: {:>4}", processing_stats.packets_per_second));

            ui.same_line(0.0);
            ui.text(format!("Bytes Per Second: {:>4}", processing_stats.bytes_per_second));

            ui.separator();

            ui.columns(5, im_str!("PacketStats"), true);

            ui.text("       Apid: ");
            ui.next_column();
            ui.text("    Count: ");
            ui.next_column();
            ui.text("  Total Bytes: ");
            ui.next_column();
            ui.text("   Byte Len:");
            ui.next_column();
            ui.text("   Last Seq:");
            ui.separator();

            for packet_stats in processing_stats.packet_history.values() {
                ui.next_column();
                ui.text(format!("      {:>5}", &packet_stats.apid.to_string()));
                packet_summary_ui(ui, &packet_stats);

                ui.next_column();
                ui.text(format!("    {:>5}", packet_stats.packet_count.to_string()));
                packet_summary_ui(ui, &packet_stats);

                ui.next_column();
                ui.text(format!("  {:>9}", &packet_stats.byte_count.to_string()));
                packet_summary_ui(ui, &packet_stats);

                ui.next_column();
                ui.text(format!("    {:>5}", &packet_stats.last_len.to_string()));
                packet_summary_ui(ui, &packet_stats);

                ui.next_column();
                ui.text(format!("    {:>5}", &packet_stats.last_seq.to_string()));
                packet_summary_ui(ui, &packet_stats);
            }

            if processing_stats.packet_history.len() > 0 {
                ui.separator();

                ui.next_column();
                ui.text(format!("         {}", processing_stats.packet_history.len()));

                ui.next_column();
                let total_count = processing_stats.packet_history.values().map(|stats: &PacketStats| stats.packet_count as u32).sum::<u32>();
                ui.text(format!("    {:>5}", total_count));

                ui.next_column();
                let total_byte_count = processing_stats.packet_history.values().map(|stats: &PacketStats| stats.byte_count).sum::<u64>();
                ui.text(format!("  {:>9}", total_byte_count));

                ui.next_column();
            }
        });
}

fn timestamp_def_ui(ui: &Ui, timestamp_def: &mut TimestampDef) {
     ui.columns(2, im_str!("TimeDefinitions"), false);
    let mut num_bytes_selection = timestamp_def.num_bytes_seconds.to_num_bytes() as i32;
    ui.input_int(im_str!("Byte For Seconds"), &mut num_bytes_selection).build();
    timestamp_def.num_bytes_seconds = TimeSize::from_num_bytes(num_bytes_selection as usize);

    ui.next_column();
    let mut num_bytes_selection = timestamp_def.num_bytes_subseconds.to_num_bytes() as i32;
    ui.input_int(im_str!("Bytes for Subsecs"), &mut num_bytes_selection).build();
    timestamp_def.num_bytes_subseconds = TimeSize::from_num_bytes(num_bytes_selection as usize);

    ui.next_column();
    ui.input_int(im_str!("Bytes Past Header"), &mut timestamp_def.offset).build();

    ui.next_column();
    ui.input_float(im_str!("Subsec Resolution"), &mut timestamp_def.subsecond_resolution).build();

    ui.next_column();
    ui.checkbox(im_str!("Little Endian"), &mut timestamp_def.is_little_endian);
    if ui.is_item_hovered() {
        ui.tooltip_text(im_str!("Decode timestamp as Little Endian (default is Big Endian)"));
    }
}

fn input_string(ui: &Ui, label: &ImStr, string: &mut String, imgui_str: &mut ImString) {
    imgui_str.clear();
    imgui_str.push_str(&string);
    ui.input_text(label, imgui_str).build();
    string.clear();
    string.push_str(&imgui_str.to_str());
}

fn input_stream_ui(ui: &Ui,
                   selection: &mut StreamOption,
                   input_settings: &mut StreamSettings,
                   allowed_apids: &mut Option<Vec<u16>>,
                   imgui_str: &mut ImString) {
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

    ui.columns(1, im_str!("default"), false); match selection {
        StreamOption::File => {
            ui.text(im_str!("Select Input File Parameters:"));
            input_string(&ui, im_str!("File Name"), &mut input_settings.file.file_name, imgui_str);
        },

        StreamOption::Udp => {
            ui.text(im_str!("Select Udp Socket Parameters:"));
            ui.columns(2, im_str!("UdpSocketCols"), false);
            input_string(&ui, im_str!("IP"), &mut input_settings.udp.ip, imgui_str);
            ui.next_column();
            input_port(&ui, &mut im_str!("Port"), &mut input_settings.udp.port);
        },

        StreamOption::TcpClient => {
            ui.text(im_str!("Select Tcp Client Parameters:"));
            ui.columns(2, im_str!("UdpSocketCols"), false);
            input_string(&ui, im_str!("IP"), &mut input_settings.tcp_client.ip, imgui_str);
            ui.next_column();
            input_port(&ui, im_str!("Port"), &mut input_settings.tcp_client.port);
        },

        StreamOption::TcpServer => {
            ui.text(im_str!("Select Tcp Server Socket Parameters:"));
            ui.columns(2, im_str!("UdpSocketCols"), false);
            input_string(&ui, im_str!("IP"), &mut input_settings.tcp_server.ip, imgui_str);
            ui.next_column();
            input_port(&ui, im_str!("Port"), &mut input_settings.tcp_server.port);
        },
    }

    filter_apids_ui(ui, allowed_apids, imgui_str);
}

fn output_stream_ui(ui: &Ui,
                    selection: &mut StreamOption,
                    output_settings: &mut StreamSettings,
                    allowed_output_apids: &mut Option<Vec<u16>>,
                    imgui_str: &mut ImString) {
    let mut input_selection: i32 = *selection as i32;

    ui.columns(5, im_str!("SelectOutput"), false);

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
            input_string(&ui, im_str!("File Name"), &mut output_settings.file.file_name, imgui_str);
        },

        StreamOption::Udp => {
            ui.text(im_str!("Select Udp Socket Parameters:"));
            ui.columns(2, im_str!("UdpSocketCols"), false);
            input_string(&ui, im_str!("IP"), &mut output_settings.udp.ip, imgui_str);
            ui.next_column();
            input_port(&ui, &mut im_str!("Port"), &mut output_settings.udp.port);
        },

        StreamOption::TcpClient => {
            ui.text(im_str!("Select Tcp Client Parameters:"));
            ui.columns(2, im_str!("UdpSocketCols"), false);
            input_string(&ui, im_str!("IP"), &mut output_settings.tcp_client.ip, imgui_str);
            ui.next_column();
            input_port(&ui, im_str!("Port"), &mut output_settings.tcp_client.port);
        },

        StreamOption::TcpServer => {
            ui.text(im_str!("Select Tcp Server Socket Parameters:"));
            ui.columns(2, im_str!("UdpSocketCols"), false);
            input_string(&ui, im_str!("IP"), &mut output_settings.tcp_server.ip, imgui_str);
            ui.next_column();
            input_port(&ui, im_str!("Port"), &mut output_settings.tcp_server.port);
        },
    }

    ui.next_column();
    filter_apids_ui(ui, allowed_output_apids, imgui_str);
}

fn filter_apids_ui(ui: &Ui, allowed_apids: &mut Option<Vec<u16>>, imgui_str: &mut ImString) {
    let mut filter_apids = allowed_apids.is_some();

    ui.checkbox(im_str!("Filter APIDs"), &mut filter_apids);
    let mut apid_list;
    if allowed_apids.is_none() {
        apid_list = Vec::new();
    } else {
        apid_list = allowed_apids.clone().unwrap();
    }

    if filter_apids {
        let mut apid_list_str: String = "".to_string();
        for apid in  apid_list.iter() {
            apid_list_str.push_str(&apid.to_string());
            apid_list_str.push(',');
        }
        input_string(&ui, im_str!("Allowed APIDs"), &mut apid_list_str, imgui_str);
        apid_list.clear();
        for apid_str in apid_list_str.split(",") {
            match apid_str.parse() {
                Ok(apid) => apid_list.push(apid),
                Err(_) => {},
            }
        }
        *allowed_apids = Some(apid_list);
    } else {
        *allowed_apids = None;
    }
}

fn load_config(file_name: &String) -> Option<AppConfig> {
    let result: Option<AppConfig>;

    match File::open(file_name) {
        Ok(file_opened) => {
            let mut file = file_opened;
            let mut config_str = String::new();

            file.read_to_string(&mut config_str).unwrap();

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
    file.write_all(&serde_json::to_string_pretty(&config).unwrap().as_bytes()).unwrap();
}

