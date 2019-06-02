use std::fs::File;
use std::io::{Read, BufReader};
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream, UdpSocket, SocketAddrV4};
use std::time::Duration;
use std::borrow::BorrowMut;

use bytes::BytesMut;
use bytes::BufMut;

use ccsds_primary_header::primary_header::*;


/// The stream option is the input/output stream type
#[derive(FromPrimitive, Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum StreamOption {
    /// The stream is a file
    File      = 1,
    /// The stream is a TCP client with a given port
    TcpClient = 2,
    /// The stream is a TCP server with a given port
    TcpServer = 3,
    /// The stream is a UDP socket with a given port
    Udp       = 4,
}

impl Default for StreamOption {
    fn default() -> Self {
        StreamOption::File
    }
}

impl StreamOption {
    pub fn open_input(&self, input_settings: &StreamSettings) -> Result<ReadStream, String> {
        let result;

        match self {
            StreamOption::File => {
                result = File::open(input_settings.file.file_name.clone())
                         .map(|file| ReadStream::File(BufReader::new(file)))
                         .map_err(|err| format!("File open error for reading: {}", err));
            },

            StreamOption::TcpClient => {
                let addr = SocketAddrV4::new(input_settings.tcp_client.ip.parse().unwrap(),
                                             input_settings.tcp_client.port);
                result = TcpStream::connect(&addr)
                         .map(|sock| ReadStream::Tcp(sock))
                         .map_err(|err| format!("TCP Client Open Error: {}", err));
            },

            StreamOption::TcpServer => {
                let addr = SocketAddrV4::new(input_settings.tcp_server.ip.parse().unwrap(),
                input_settings.tcp_server.port);
                let listener = TcpListener::bind(&addr).unwrap();
                let (sock, _) = listener.accept().map_err(|err| format!("TCP Server Open Error: {}", err))?;
                result = Ok(ReadStream::Tcp(sock));
            },

            StreamOption::Udp => {
                let sock = UdpSocket::bind("0.0.0.0:0").map_err(|err| "couldn't bind to udp address/port")?;
                result = Ok(ReadStream::Udp(sock));
            },
        }

        result
    }

    pub fn open_output(&self, output_settings: &StreamSettings) -> Result<WriteStream, String> {
        let result: Result<WriteStream, String>;

        match self {
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

}

/* Input Streams */
/// The file settings are everything needed to open and read from a file as an input or output
/// stream
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSettings {
    pub file_name: String,
}

impl Default for FileSettings {
    fn default() -> Self {
        FileSettings { file_name: "data.bin".to_string() }
    }
}

/// The tcp client settings are everything needed to open and read from a tcp socket as an input or output
/// stream as a tcp client
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

/// The tcp server settings are everything needed to open and read from a tcp socket as an input or output
/// stream as a tcp server
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

/// The udp settings are everything needed to open a UDP socket and use it as an input or output
/// stream
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

/// The stream settings are all the settings for all stream types
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamSettings {
    #[serde(default)]
    pub file: FileSettings,

    #[serde(default)]
    pub tcp_client: TcpClientSettings,

    #[serde(default)]
    pub tcp_server: TcpServerSettings,

    #[serde(default)]
    pub udp: UdpSettings,
}

/* Input/Output Streams */
/// A read stream a source of CCSDS packets
#[derive(Debug)]
pub enum ReadStream {
    File(BufReader<File>),
    Udp(UdpSocket),
    Tcp(TcpStream),
    Null,
}

impl ReadStream {
    pub fn stream_read(&mut self,
                       bytes: &mut BytesMut,
                       num_bytes: usize) -> Result<usize, String> {

        let result: Result<usize, String>;

        match self {
            ReadStream::File(ref mut file) => {
                result = read_bytes(file, bytes, num_bytes);
            },

            ReadStream::Udp(udp_sock) => {
                // for UDP we just read a message, which must contain a CCSDS packet
                bytes.clear();
                result = udp_sock.recv(bytes).map_err(|err| format!("Udp Socket Read Error: {}", err));
            },

            ReadStream::Tcp(tcp_stream) => {
                result = read_bytes(tcp_stream, bytes, num_bytes);
            },

            ReadStream::Null => {
                result = Err("Reading a Null Stream! This should not happen!".to_string());
            },
        }

        result
    }
}


/// A read stream a sink of CCSDS packets
#[derive(Debug)]
pub enum WriteStream {
    File(File),
    Udp((UdpSocket, SocketAddrV4)),
    Tcp(TcpStream),
    Null,
}

impl WriteStream {
    pub fn stream_send(&mut self, packet: &Vec<u8>) -> Result<(), String> {
        match self {
            WriteStream::File(file) => {
                file.write_all(&packet).map_err(|err| format!("IO error {}", err))
            },

            WriteStream::Udp((udp_sock, addr)) => {
                udp_sock.send_to(&packet, &*addr)
                        .map_err(|err| format!("IO error {}", err))
                        .map(|_| ())
            },

            WriteStream::Tcp(tcp_stream) => {
                tcp_stream.write_all(&packet).map_err(|err| format!("IO error {}", err))
            },

            WriteStream::Null => {
                Ok(())
            },
        }
    }
}


/// The packet structure contains the data for a packet, as well as the primary header
#[derive(Debug, Clone)]
pub struct Packet {
    pub header: CcsdsPrimaryHeader,
    pub bytes:  Vec<u8>,
}


fn read_bytes<R: Read>(reader: &mut R, bytes: &mut BytesMut, num_bytes: usize) -> Result<usize, String> {
    let current_len = bytes.len();

    bytes.reserve(num_bytes);

    let mut_bytes: &mut [u8] = bytes.borrow_mut();
    reader.read_exact(&mut mut_bytes[current_len..(current_len + num_bytes)])
          .map_err(|err| format!("Stream Read Error: {}", err))?;

    Ok(num_bytes)
}

