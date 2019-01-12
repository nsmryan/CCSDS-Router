use std::fs::File;
use std::io::{Read, BufReader, Write};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream, UdpSocket, SocketAddrV4};

use ccsds_primary_header::*;

use byteorder::{LittleEndian};


#[derive(FromPrimitive, Debug, Copy, Clone, Serialize, Deserialize)]
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
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct FileSettings {
    pub file_name: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TcpClientSettings {
    pub port: u16,
    pub ip: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TcpServerSettings {
    pub port: u16,
    pub ip: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct UdpSettings {
    pub port: u16,
    pub ip: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
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


pub fn open_read_stream(input_settings: &StreamSettings, input_option: StreamOption) -> Option<ReadStream> {
    let stream: ReadStream;

    match input_option {
        StreamOption::File => {
            let mut file = File::open(input_settings.file.file_name.clone()).unwrap();
            let mut file = BufReader::new(file);

            stream = ReadStream::File(file);
        },

        StreamOption::TcpClient => {
            let addr = SocketAddrV4::new(input_settings.tcp_client.ip.parse().unwrap(),
            input_settings.tcp_client.port);
            let stream_conn = TcpStream::connect(&addr);
            match stream_conn {
                Ok(mut sock) => {
                    stream = ReadStream::Tcp(sock);
                }, 
                Err(_) => unreachable!(),
            }
        },

        StreamOption::TcpServer => {
            let addr = SocketAddrV4::new(input_settings.tcp_server.ip.parse().unwrap(),
            input_settings.tcp_server.port);
            let listener = TcpListener::bind(&addr).unwrap();
            match listener.accept() {
                Ok((mut sock, _)) => {
                    stream = ReadStream::Tcp(sock);
                }, 
                Err(_) => unreachable!(),
            }
        },

        StreamOption::Udp => {
            let sock = UdpSocket::bind("0.0.0.0:0").expect("couldn't bind to udp address/port");
            stream = ReadStream::Udp(sock);
        },
    }

    Some(stream)
}

pub fn open_write_stream(output_settings: &StreamSettings, output_option: StreamOption) -> Option<WriteStream> {
    let stream: WriteStream;
    match output_option {
        StreamOption::File => {
            let outfile = File::create(output_settings.file.file_name.clone()).unwrap();
            stream = WriteStream::File(outfile);
        },

        StreamOption::TcpClient => {
            let addr = SocketAddrV4::new(output_settings.tcp_client.ip.parse().unwrap(),
            output_settings.tcp_client.port);
            let stream_conn = TcpStream::connect(&addr);
            match stream_conn {
                Ok(mut sock) => {
                    stream = WriteStream::Tcp(sock);
                }, 

                Err(_) => unreachable!(),
            }
        },

        StreamOption::TcpServer => {
            let addr = SocketAddrV4::new(output_settings.tcp_server.ip.parse().unwrap(),
            output_settings.tcp_server.port);
            let listener = TcpListener::bind(&addr).unwrap();
            match listener.accept() {
                Ok((mut sock, _)) => {
                    stream = WriteStream::Tcp(sock);
                }, 

                Err(_) => unreachable!(),
            }
        },

        StreamOption::Udp => {
            let addr = SocketAddrV4::new(output_settings.udp.ip.parse().unwrap(), output_settings.udp.port);
            stream = WriteStream::Udp((UdpSocket::bind("0.0.0.0:0").expect("couldn't bind to udp address/port"), addr));
        },
    }

    Some(stream)
}

pub fn stream_read_packet(input_stream: &mut ReadStream, packet: &mut Vec<u8>) {
    let mut header_bytes: [u8;6] = [0; 6];

    match input_stream {
        ReadStream::File(ref mut file) => {
            file.read_exact(&mut header_bytes).unwrap();

            let pri_header = PrimaryHeader::<LittleEndian>::new(header_bytes);

            let data_size = pri_header.packet_length() - CCSDS_PRI_HEADER_SIZE_BYTES;

            // put header in packet buffer, swapping endianness.
            packet.clear();
            packet.push(header_bytes[1]);
            packet.push(header_bytes[0]);
            packet.push(header_bytes[3]);
            packet.push(header_bytes[2]);
            packet.push(header_bytes[5]);
            packet.push(header_bytes[4]);

            for _ in 0..data_size {
                // NOTE awkward way to read. should get a slice of the vector?
                let mut byte: [u8;1] = [0; 1];
                file.read_exact(&mut byte).unwrap();
                packet.push(byte[0]);
            }
        },

        ReadStream::Udp(udp_sock) => {
            // for UDP we just read a message, which must contain a CCSDS packet
            packet.clear();
            udp_sock.recv(packet).unwrap();
        },

        ReadStream::Tcp(tcp_stream) => {
            // TODO implement for TCP
            packet.clear();
        },

        ReadStream::Null() => {
            packet.clear();
        },
    }
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
