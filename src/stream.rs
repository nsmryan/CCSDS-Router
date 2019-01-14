use std::fs::File;
use std::io::{Read, BufReader};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream, UdpSocket, SocketAddrV4};

use ccsds_primary_header::*;

use types::*;


#[derive(FromPrimitive, Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum StreamOption {
    File      = 1,
    TcpClient = 2,
    TcpServer = 3,
    Udp       = 4,
}

impl Default for StreamOption {
    fn default() -> Self {
        StreamOption::File
    }
}

/* Input Streams */
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSettings {
    pub file_name: String,
}

impl Default for FileSettings {
    fn default() -> Self {
        FileSettings { file_name: "data.bin".to_string() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpClientSettings {
    pub port: u16,
    pub ip: String,
}

impl Default for TcpClientSettings {
    fn default() -> Self {
        TcpClientSettings { port: 8000,
                            ip: "127.0.0.1".to_string()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpServerSettings {
    pub port: u16,
    pub ip: String,
}

impl Default for TcpServerSettings {
    fn default() -> Self {
        TcpServerSettings { port: 8000,
                            ip: "127.0.0.1".to_string()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UdpSettings {
    pub port: u16,
    pub ip: String,
}

impl Default for UdpSettings {
    fn default() -> Self {
        UdpSettings { port: 8001,
                      ip: "127.0.0.1".to_string()
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamSettings {
    pub file: FileSettings,
    pub tcp_client: TcpClientSettings,
    pub tcp_server: TcpServerSettings,
    pub udp: UdpSettings,
}

/* Input/Output Streams */
#[derive(Debug)]
pub enum ReadStream {
    File(BufReader<File>),
    Udp(UdpSocket),
    Tcp(TcpStream),
    Null,
}

#[derive(Debug)]
pub enum WriteStream {
    File(File),
    Udp((UdpSocket, SocketAddrV4)),
    Tcp(TcpStream),
    Null,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Endianness {
    Big,
    Little,
}

#[derive(Debug)]
pub struct Packet {
    pub header: CcsdsPrimaryHeader,
    pub bytes:  Vec<u8>,
}


pub fn open_input_stream(input_settings: &StreamSettings, input_option: StreamOption) -> Result<ReadStream, String> {
    let result;

    match input_option {
        StreamOption::File => {
            match File::open(input_settings.file.file_name.clone()) {
                Ok(file) => {
                    let mut file = BufReader::new(file);
                    result = Ok(ReadStream::File(file));
                },

                Err(e) => {
                    result = Err(format!("File open error for reading: {}", e));
                },
            }
        },

        StreamOption::TcpClient => {
            let addr = SocketAddrV4::new(input_settings.tcp_client.ip.parse().unwrap(),
            input_settings.tcp_client.port);
            let stream_conn = TcpStream::connect(&addr);
            match stream_conn {
                Ok(mut sock) => {
                    result = Ok(ReadStream::Tcp(sock));
                }, 

                Err(e) => {
                    result = Err(format!("TCP Client Open Error: {}", e));
                },
            }
        },

        StreamOption::TcpServer => {
            let addr = SocketAddrV4::new(input_settings.tcp_server.ip.parse().unwrap(),
            input_settings.tcp_server.port);
            let listener = TcpListener::bind(&addr).unwrap();
            match listener.accept() {
                Ok((mut sock, _)) => {
                    result = Ok(ReadStream::Tcp(sock));
                }, 

                Err(e) => {
                    result = Err(format!("TCP Server Open Error: {}", e));
                },
            }
        },

        StreamOption::Udp => {
            let sock = UdpSocket::bind("0.0.0.0:0").expect("couldn't bind to udp address/port");
            result = Ok(ReadStream::Udp(sock));
        },
    }

    result
}

pub fn open_output_stream(output_settings: &StreamSettings, output_option: StreamOption) -> Result<WriteStream, String> {
    let result: Result<WriteStream, String>;

    match output_option {
        StreamOption::File => {
            match File::create(output_settings.file.file_name.clone()) {
                Ok(outfile) => {
                    result = Ok(WriteStream::File(outfile));
                },

                Err(e) => {
                    result = Err(format!("File open error for writing: {}", e));
                },
            }
        },

        StreamOption::TcpClient => {
            let addr = SocketAddrV4::new(output_settings.tcp_client.ip.parse().unwrap(),
            output_settings.tcp_client.port);
            let stream_conn = TcpStream::connect(&addr);
            match stream_conn {
                Ok(mut sock) => {
                    result = Ok(WriteStream::Tcp(sock));
                }, 

                Err(e) => {
                    result = Err(format!("TCP Client Open Error: {}", e));
                },
            }
        },

        StreamOption::TcpServer => {
            let addr = SocketAddrV4::new(output_settings.tcp_server.ip.parse().unwrap(),
            output_settings.tcp_server.port);
            let listener = TcpListener::bind(&addr).unwrap();

            match listener.accept() {
                Ok((mut sock, _)) => {
                    result = Ok(WriteStream::Tcp(sock));
                }, 

                Err(e) => {
                    result = Err(format!("TCP Server Open Error: {}", e));
                },
            }
        },

        StreamOption::Udp => {
            match output_settings.udp.ip.parse() {
                Ok(ip_addr) => {
                    let addr = SocketAddrV4::new(ip_addr, output_settings.udp.port);

                    match UdpSocket::bind("0.0.0.0:0") {
                        Ok(udp_sock) => {
                            result = Ok(WriteStream::Udp((udp_sock, addr)));
                        },

                        Err(e) => {
                            result = Err(format!("Could not open UDP socket for writing: {}", e));
                        },
                    }
                },

                Err(e) => {
                    result = Err(format!("Could not parse ip ({}): {}", output_settings.udp.ip, e));
                },
            }
        },
    }

    result
}

fn read_bytes<R: Read>(reader: &mut R, bytes: &mut Vec<u8>, num_bytes: usize) -> Result<usize, String> {
    let mut result: Result<usize, String> = Ok(num_bytes);

   // NOTE awkward way to read. should get a slice of the vector?
   let mut byte: [u8;1] = [0; 1];

    for _ in 0..num_bytes {
        match reader.read_exact(&mut byte) {
            Ok(()) => {
                bytes.push(byte[0]);
            }

            Err(e) => {
                result = Err(format!("Stream Read Error: {}", e));
                break;
            }
        }
    }

    result
}

// read a packet from a stream (a file or a TCP stream)
fn read_packet_from_reader<R>(reader: &mut R,
                              packet: &mut Packet,
                              endianness: Endianness,
                              packet_size: PacketSize,
                              frame_settings: &FrameSettings) -> Result<usize, String>
    where R: Read {

    let result: Result<usize, String>;

    let mut header_bytes: [u8; CCSDS_PRI_HEADER_SIZE_BYTES as usize] = [0; CCSDS_PRI_HEADER_SIZE_BYTES as usize];

    let mut packet_length_bytes = 0;

    packet.bytes.clear();

    match read_bytes(reader, &mut packet.bytes, frame_settings.prefix_bytes as usize) {
        Ok(_) => {
            // if we do not keep the prefix, clear the packet before continueing
            if !frame_settings.keep_prefix {
                packet.bytes.clear();
            } else {
                packet_length_bytes += frame_settings.prefix_bytes;
            }

            // read enough bytes for a header
            match reader.read_exact(&mut header_bytes) {
                Ok(()) => {
                    packet_length_bytes += CCSDS_PRI_HEADER_SIZE_BYTES as i32;

                    // put header in packet buffer
                    packet.bytes.extend_from_slice(&header_bytes);

                    // if the header is laid out little endian, swap to big endian
                    // before using as a CCSDS header
                    if endianness == Endianness::Little {
                        for byte_index in 0..3 {
                            let tmp = header_bytes[byte_index * 2];
                            header_bytes[byte_index * 2] = header_bytes[byte_index * 2 + 1];
                            header_bytes[byte_index * 2 + 1] = tmp;
                        }
                    }
                    packet.header = CcsdsPrimaryHeader::new(header_bytes);

                    // the packet length is either in the header, or it is given by the caller
                    let packet_length = 
                        match packet_size {
                            PacketSize::Variable => packet.header.packet_length(),
                            PacketSize::Fixed(num_bytes) => num_bytes,
                        };

                    let data_size = packet_length - CCSDS_PRI_HEADER_SIZE_BYTES;

                    // read the data section of the packet
                    match read_bytes(reader, &mut packet.bytes, data_size as usize) {
                        Err(e) => result = Err(e),

                        _ => {
                            packet_length_bytes += CCSDS_PRI_HEADER_SIZE_BYTES as i32;
                            // read the data section of the packet
                            match read_bytes(reader, &mut packet.bytes, frame_settings.postfix_bytes as usize) {
                                Err(e) => result = Err(e),

                                _ => {
                                    // if we are not keeping the postfix bytes, truncate to the previous
                                    // size.
                                    if !frame_settings.keep_postfix {
                                        packet.bytes.truncate(packet_length_bytes as usize);
                                    } else {
                                        // otherwise keep the postfix bytes
                                        packet_length_bytes += CCSDS_PRI_HEADER_SIZE_BYTES as i32;
                                    }

                                    result = Ok(packet_length_bytes as usize);
                                },
                            }
                        },
                    }
                },

                Err(e) => {
                    result = Err(format!("Stream Read Error: {}", e));
                },
            }
        },

        Err(err) => result = Err(err),
    }

    result
}

pub fn stream_read_packet(input_stream: &mut ReadStream,
                          packet: &mut Packet,
                          endianness: Endianness,
                          packet_size: PacketSize,
                          frame_settings: &FrameSettings) -> Result<usize, String> {

    let result: Result<usize, String>;

    match input_stream {
        ReadStream::File(ref mut file) => {
            result = read_packet_from_reader(file, packet, endianness, packet_size, frame_settings);
        },

        ReadStream::Udp(udp_sock) => {
            // for UDP we just read a message, which must contain a CCSDS packet
            // NOTE currently ignores packet size
            packet.bytes.clear();
            match udp_sock.recv(&mut packet.bytes) {
                Ok(bytes_read) => {
                    result = Ok(bytes_read);
                },

                Err(e) => {
                    result = Err(format!("Udp Socket Read Error: {}", e));
                },
            }
        },

        ReadStream::Tcp(tcp_stream) => {
            result = read_packet_from_reader(tcp_stream, packet, endianness, packet_size, frame_settings);
        },

        ReadStream::Null => {
            result = Err("Reading a Null Stream! This should not happen!".to_string());
        },
    }

    result
}

pub fn stream_send(output_stream: &mut WriteStream, packet: &Vec<u8>) {
    match output_stream {
        WriteStream::File(file) => {
            file.write_all(&packet).unwrap();
        },

        WriteStream::Udp((udp_sock, addr)) => {
            udp_sock.send_to(&packet, &*addr).unwrap();
        },

        WriteStream::Tcp(tcp_stream) => {
            tcp_stream.write_all(&packet).unwrap();
        },

        WriteStream::Null => {

        },
    }
}
