extern crate ccsds_primary_header;

extern crate bytes;
extern crate byteorder;

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

enum StreamOption {
    File      = 1,
    TcpClient = 2,
    TcpServer = 3,
    Udp       = 4,
}

/* Input Streams */
struct FileSettings {
    file_name: String,
}

struct TcpClientSettings {
    port: u16,
    ip: String,
}

struct TcpServerSettings {
    port: u16,
    ip: String,
}

struct UdpSettings {
    port: u16,
    ip: String,
}

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

struct PacketStats {
    apid: Apid,
    packet_count: u16,
    byte_count: u32,
}

impl Default for PacketStats {

    fn default() -> PacketStats {
      PacketStats { apid: 0,
                    packet_count: 0,
                    byte_count: 0 }
    }
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
        StreamOption::File      => {
           let outfile = File::create(input_settings.file.file_name).unwrap();
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

        StreamOption::Udp       => {
          let addr = SocketAddrV4::new(input_settings.udp.ip.parse().unwrap(),
                                       input_settings.udp.port);
          stream = Stream::Udp((UdpSocket::bind(&addr).expect("couldn't bind to udp address/port"), addr));
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

  /* UDP Socket */
  let sock = UdpSocket::bind("127.0.0.1:8000").expect("couldn't bind to address");

  let (sender, receiver) = channel::<ProcessingMsg>();

  let addr = SocketAddrV4::new("127.0.0.1".parse().unwrap(),
                               8001);

  let mut stream = Stream::Udp((sock, addr));

  //let stream_settings: StreamSettings = Default::default();
  //let mut stream = open_stream(&stream_settings, StreamOption::Udp);

  let ccsds_thread = thread::spawn(move || {
      stream_ccsds( &mut buf, &mut stream, sender );
  });

  run_gui(receiver);

  ccsds_thread.join().unwrap();
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


  let mut imgui_sdl2 = imgui_sdl2::ImguiSdl2::new(&mut imgui);

  let renderer = imgui_opengl_renderer::Renderer::new(&mut imgui, |s| video.gl_get_proc_address(s) as _);

  let mut event_pump = sdl_context.event_pump().unwrap();

  let mut packet_history: PacketHistory = HashMap::new();

  let mut exit_gui = false;

  let mut input_selection: i32 = 0;

  let mut input_file_name: ImString = ImString::with_capacity(256);

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
            ui.child_frame(im_str!("Select Input Type"), (WINDOW_WIDTH - 15.0, 75.0))
                .show_borders(true)
                .build(|| {
                    ui.columns(4, im_str!("SelectInputType"), false);
                    ui.radio_button(im_str!("File"),       &mut input_selection, 1);
                    ui.next_column();
                    ui.radio_button(im_str!("UDP"),        &mut input_selection, 2);
                    ui.next_column();
                    ui.radio_button(im_str!("TCP Client"), &mut input_selection, 3);
                    ui.next_column();
                    ui.radio_button(im_str!("TCP Server"), &mut input_selection, 4);

                    ui.columns(1, im_str!("default"), false);
                    if input_selection == 1 {
                        ui.text(im_str!("Select Input File Parameters:"));
                        ui.input_text(im_str!("File Name"), &mut input_file_name).build();
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

                    for packet_stats in packet_history.values() {
                        let mut string = String::from("Apid: ");
                        string.push_str(&format!("{:>5}", &packet_stats.apid.to_string()));
                        string.push_str(&format!(", Count: {:>5}", packet_stats.packet_count.to_string()));
                        ui.text(string.as_str());
                    }
            });

            //ui.plot_lines(im_str!("Bytes Processed"), &bytes_processed_vec[..]).build();

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
