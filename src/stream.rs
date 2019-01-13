use std::fs::File;
use std::io::{Read, BufReader};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream, UdpSocket, SocketAddrV4};

use ccsds_primary_header::*;

use byteorder::{ByteOrder, BigEndian, LittleEndian};


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
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSettings {
    pub file_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpClientSettings {
    pub port: u16,
    pub ip: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpServerSettings {
    pub port: u16,
    pub ip: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UdpSettings {
    pub port: u16,
    pub ip: String,
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
    Null(),
}

#[derive(Debug)]
pub enum WriteStream {
    File(File),
    Udp((UdpSocket, SocketAddrV4)),
    Tcp(TcpStream),
    Null(),
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

#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum PacketSize {
    Variable,
    Fixed(u16),
}

impl Default for PacketSize {
    fn default() -> Self {
        PacketSize::Variable
    }
}

impl PacketSize {
    pub fn num_bytes(&self) -> u16 {
        match self {
            PacketSize::Variable => 100,

            PacketSize::Fixed(num) => *num,
        }
    }
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

// read a packet from a stream (a file or a TCP stream)
fn read_packet_from_reader<R>(reader: &mut R, packet: &mut Packet, endianness: Endianness, packet_size: PacketSize) ->
    Result<usize, String> where R: Read {

    let mut result: Result<usize, String>;

    let mut header_bytes: [u8;6] = [0; 6];

    // read enough bytes for a header
    match reader.read_exact(&mut header_bytes) {
        Ok(()) => {
            // if the header is laid out little endian, swap to big endian
            if endianness == Endianness::Little {
                for byte_index in 0..3 {
                    let tmp = header_bytes[byte_index * 2];
                    header_bytes[byte_index * 2] = header_bytes[byte_index * 2 + 1];
                    header_bytes[byte_index * 2 + 1] = tmp;
                }
            }

            packet.header = CcsdsPrimaryHeader::new(header_bytes);

            let packet_length;
            match packet_size {
                PacketSize::Variable => packet_length = packet.header.packet_length(),
                PacketSize::Fixed(num_bytes) => packet_length = num_bytes,
            }

            let data_size = packet_length - CCSDS_PRI_HEADER_SIZE_BYTES;

            // put header in packet buffer, swapping endianness.
            packet.bytes.clear();
            packet.bytes.push(header_bytes[1]);
            packet.bytes.push(header_bytes[0]);
            packet.bytes.push(header_bytes[3]);
            packet.bytes.push(header_bytes[2]);
            packet.bytes.push(header_bytes[5]);
            packet.bytes.push(header_bytes[4]);

            result = Ok(packet_length as usize);

            // read the rest of the packet
            for _ in 0..data_size {
                // NOTE awkward way to read. should get a slice of the vector?
                let mut byte: [u8;1] = [0; 1];
                match reader.read_exact(&mut byte) {
                    Ok(()) => {
                        packet.bytes.push(byte[0]);
                    }

                    Err(e) => {
                        result = Err(format!("Stream Read Error: {}", e));
                        break;
                    }
                }
            }
        },

        Err(e) => {
            result = Err(format!("Stream Read Error: {}", e));
        },
    }

    result
}

pub fn stream_read_packet(input_stream: &mut ReadStream, packet: &mut Packet, endianness: Endianness, packet_size: PacketSize) ->
    Result<usize, String> {

    let mut result: Result<usize, String>;

    match input_stream {
        ReadStream::File(ref mut file) => {
            result = read_packet_from_reader(file, packet, endianness, packet_size);
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
            result = read_packet_from_reader(tcp_stream, packet, endianness, packet_size);
        },

        ReadStream::Null() => {
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

        WriteStream::Null() => {

        },
    }
}
