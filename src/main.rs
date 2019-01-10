extern crate ccsds_primary_header;

extern crate bytes;
extern crate byteorder;

extern crate num;
#[macro_use] extern crate num_derive;

extern crate sdl2;
extern crate imgui;
extern crate imgui_sdl2;
extern crate gl;
extern crate imgui_opengl_renderer;


use std::time;
use std::thread;
use std::fs::File;
use std::io::Read;
use std::io::prelude::*;
use std::default::Default;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::net::{TcpListener, TcpStream, UdpSocket, SocketAddrV4};

use ccsds_primary_header::*;

use bytes::{Bytes, Buf};
use bytes::buf::IntoBuf;

use byteorder::{LittleEndian};

use imgui::*;


const WINDOW_WIDTH: f32 = 640.0;
const WINDOW_HEIGHT: f32 = 480.0;

type Apid = u16;

#[derive(FromPrimitive)]
enum StreamOption {
    File      = 1,
    TcpClient = 2,
    TcpServer = 3,
    Udp       = 4,
}

/* Input Streams */
#[derive(Default)]
struct FileSettings {
    file_name: String,
}

#[derive(Default)]
struct TcpClientSettings {
    port: u16,
    ip: String,
}

#[derive(Default)]
struct TcpServerSettings {
    port: u16,
    ip: String,
}

#[derive(Default)]
struct UdpSettings {
    port: u16,
    ip: String,
}

#[derive(Default)]
struct StreamSettings {
    file: FileSettings,
    tcp_client: TcpClientSettings,
    tcp_server: TcpServerSettings,
    udp: UdpSettings,
}

/* Input/Output Streams */
enum Stream {
    File(File),
    Udp((UdpSocket, SocketAddrV4)),
    Tcp(TcpStream),
}

/* Packet Data */
type PacketHistory = HashMap<Apid, PacketStats>;

#[derive(Default)]
struct PacketStats {
    apid: Apid,
    packet_count: u16,
    byte_count: u32,
}

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
enum ProcessingMsg {
    PacketUpdate(PacketUpdate),
    Terminate(),
}

fn open_stream(input_settings: &StreamSettings, input_option: StreamOption) -> Stream {
    let stream: Stream;
    match input_option {
        StreamOption::File => {
           let outfile = File::create(input_settings.file.file_name.clone()).unwrap();
            stream = Stream::File(outfile);
        },

        StreamOption::TcpClient => {
          let addr = SocketAddrV4::new(input_settings.tcp_client.ip.parse().unwrap(),
                                       input_settings.tcp_client.port);
          let stream_conn = TcpStream::connect(&addr);
          match stream_conn {
              Ok(mut sock) => {
                  stream = Stream::Tcp(sock);
              }, 
              Err(e) => unreachable!(),
          }
        },

        StreamOption::TcpServer => {
          let addr = SocketAddrV4::new(input_settings.tcp_server.ip.parse().unwrap(),
                                       input_settings.tcp_server.port);
          let listener = TcpListener::bind(&addr).unwrap();
          match listener.accept() {
              Ok((mut sock, addr)) => {
                  stream = Stream::Tcp(sock);
              }, 
              Err(e) => unreachable!(),
          }
        },

        StreamOption::Udp => {
          let addr = SocketAddrV4::new(input_settings.udp.ip.parse().unwrap(),
                                       input_settings.udp.port);
          stream = Stream::Udp((UdpSocket::bind("0.0.0.0:0").expect("couldn't bind to udp address/port"), addr));
        },
    }

    stream
}


fn send_packet(output_stream: &mut Stream, packet: &Vec<u8>) {
    match output_stream {
      Stream::File(file) => {
          file.write_all(&packet);
      },

      Stream::Udp((udp_sock, addr)) => {
         udp_sock.send_to(&packet, &*addr).unwrap();
      },

      Stream::Tcp(tcp_stream) => {
          tcp_stream.write_all(&packet);
      },
    }
}

fn main() {

  let mut file = File::open("Logged_Data").unwrap();

  let mut byte_vec = Vec::new();
  
  file.read_to_end(&mut byte_vec).unwrap();

  
  let bytes = Bytes::from(byte_vec);
  let mut buf = bytes.into_buf();

  let (sender, receiver) = channel::<ProcessingMsg>();

  let mut stream_settings: StreamSettings = Default::default();

  stream_settings.udp.port = 8001;
  stream_settings.udp.ip = "127.0.0.1".to_string();

  let mut stream = open_stream(&stream_settings, StreamOption::Udp);

  let ccsds_thread = thread::spawn(move || {
      stream_ccsds( &mut buf, &mut stream, sender );
  });

  run_gui(receiver);

  ccsds_thread.join().unwrap();
}

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
            //ImVec4::new(0.01, 0.01, 0.01, 0.86), // ImGuiCol_Tab 
            //ImVec4::new(0.29, 0.29, 0.29, 1.00), // ImGuiCol_TabHovered 
            //ImVec4::new(0.31, 0.31, 0.31, 1.00), // ImGuiCol_TabActive 
            //ImVec4::new(0.02, 0.02, 0.02, 1.00), // ImGuiCol_TabUnfocused 
            //ImVec4::new(0.19, 0.19, 0.19, 1.00), // ImGuiCol_TabUnfocusedActive 
            //ImVec4::new(0.38, 0.48, 0.60, 1.00), // ImGuiCol_DockingPreview 
            //ImVec4::new(0.20, 0.20, 0.20, 1.00), // ImGuiCol_DockingEmptyBg 
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

fn run_gui(receiver: Receiver<ProcessingMsg>) {
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

  let mut packet_history: PacketHistory = HashMap::new();

  let mut exit_gui = false;

  let mut input_selection: i32 = 1;

  let mut input_file_name: ImString = ImString::with_capacity(256);
  let mut udp_ip_addr : ImString = ImString::with_capacity(256);
  let mut tcp_client_ip_addr : ImString = ImString::with_capacity(256);
  let mut tcp_server_ip_addr : ImString = ImString::with_capacity(256);

  let mut udp_port : i32 = 8000;
  let mut tcp_client_port : i32 = 8000;
  let mut tcp_server_port : i32 = 8000;

  let mut bytes_processed: u32 = 0;
  let mut bytes_processed_vec: Vec<f32> = Vec::new();

  'running: loop {
    if exit_gui {
        break 'running;
    }

    use sdl2::event::Event;
    use sdl2::keyboard::Keycode;

    for event in event_pump.poll_iter() {
      imgui_sdl2.handle_event(&mut imgui, &event);
      if imgui_sdl2.ignore_event(&event) { continue; }

      match event {
        Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
          break 'running
        },
        _ => {}
      }
    }

    while let Ok(msg) = receiver.recv_timeout(time::Duration::from_millis(0)) {
        match msg {
            ProcessingMsg::Terminate() => break,

            ProcessingMsg::PacketUpdate(packet_update) => {
                bytes_processed += packet_update.packet_length as u32;
                bytes_processed_vec.push(bytes_processed as f32);

                packet_history.entry(packet_update.apid)
                              .or_default()
                              .update(packet_update);
            },
        }
    }

    let ui = imgui_sdl2.frame(&window, &mut imgui, &event_pump);

    ui.window(im_str!(""))
        .position((0.0, 0.0), ImGuiCond::FirstUseEver)
        .size((WINDOW_WIDTH, WINDOW_HEIGHT), ImGuiCond::FirstUseEver)
        .title_bar(false)
        .build(|| {
            /* Source Selection */
            ui.child_frame(im_str!("Select Input Type"), (WINDOW_WIDTH - 15.0, 125.0))
                .show_borders(true)
                .build(|| {
                    ui.columns(4, im_str!("SelectInputType"), false);
                    ui.radio_button(im_str!("File"),       &mut input_selection, StreamOption::File as i32);
                    ui.next_column();
                    ui.radio_button(im_str!("UDP"),        &mut input_selection, StreamOption::Udp as i32);
                    ui.next_column();
                    ui.radio_button(im_str!("TCP Client"), &mut input_selection, StreamOption::TcpClient as i32);
                    ui.next_column();
                    ui.radio_button(im_str!("TCP Server"), &mut input_selection, StreamOption::TcpServer as i32);

                    ui.columns(1, im_str!("default"), false);
                    match num::FromPrimitive::from_i32(input_selection) {
                      Some(StreamOption::File) => {
                            ui.text(im_str!("Select Input File Parameters:"));
                            ui.input_text(im_str!("File Name"), &mut input_file_name).build();
                        },

                      Some(StreamOption::Udp) => {
                            ui.text(im_str!("Select Udp Socket Parameters:"));
                            ui.input_text(im_str!("IP Address"), &mut udp_ip_addr).build();
                            ui.input_int(im_str!("Port"), &mut udp_port).build();
                      },

                      Some(StreamOption::TcpClient) => {
                            ui.text(im_str!("Select Tcp Client Parameters:"));
                            ui.input_text(im_str!("IP Address"), &mut tcp_client_ip_addr).build();
                            ui.input_int(im_str!("Port"), &mut tcp_client_port).build();
                      },

                      Some(StreamOption::TcpServer) => {
                            ui.text(im_str!("Select Tcp Server Socket Parameters:"));
                            ui.input_text(im_str!("IP Address"), &mut tcp_server_ip_addr).build();
                            ui.input_int(im_str!("Port"), &mut tcp_server_port).build();
                      },

                      None => {},
                    }
                });


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

            //ui.plot_lines(im_str!("Bytes Processed"), &bytes_processed_vec[..]).build();

            if ui.small_button(im_str!("Clear Stats")) {
                packet_history.clear();
            }

            ui.same_line(0.0);

            if ui.small_button(im_str!("Exit")) {
                exit_gui = true;
            }
        });


    unsafe {
      gl::ClearColor(0.2, 0.2, 0.2, 1.0);
      gl::Clear(gl::COLOR_BUFFER_BIT);
    }

    renderer.render(ui);

    window.gl_swap_window();


    ::std::thread::sleep(::std::time::Duration::new(0, 1_000_000_000u32 / 60));
  }
}

fn stream_ccsds<B>(buf: &mut B, output_stream: &mut Stream, sender: Sender<ProcessingMsg>) 
  where B: Buf {

  let mut packet: Vec<u8> = Vec::with_capacity(4096);

  let mut header_bytes: [u8;6] = [0; 6];

  while buf.remaining() >= CCSDS_MIN_LENGTH as usize {
      buf.copy_to_slice(&mut header_bytes);

      packet.clear();
      packet.push(header_bytes[1]);
      packet.push(header_bytes[0]);
      packet.push(header_bytes[3]);
      packet.push(header_bytes[2]);
      packet.push(header_bytes[5]);
      packet.push(header_bytes[4]);

      let pri_header = PrimaryHeader::<LittleEndian>::new(header_bytes);

      let data_size = pri_header.packet_length() - CCSDS_PRI_HEADER_SIZE_BYTES;

      if buf.remaining() < data_size as usize {
          break;
      }

      for _ in 0..data_size {
          packet.push(buf.get_u8());
      }

      send_packet(output_stream, &packet);

      sender.send(ProcessingMsg::PacketUpdate(PacketUpdate
                                { apid: pri_header.control.apid(),
                                  packet_length: packet.len() as u16,
                                }
      )).unwrap();
      sender.send(ProcessingMsg::Terminate()).unwrap();
  }
}
