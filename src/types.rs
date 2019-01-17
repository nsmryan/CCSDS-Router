use std::time::{Duration, SystemTime};
use std::default::Default;
use std::collections::HashMap;

use ccsds_primary_header::*;


use stream::*;


/// Apid from CCSDS standard
type Apid = u16;

/// The GuiTheme to use with ImGui
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum GuiTheme {
    Dark,
    Light,
}

impl Default for GuiTheme {
    fn default() -> Self {
        GuiTheme::Dark
    }
}

/* Application Configuration */
/// Application configuration contains all configuration required to processing
/// inputs and outputs. This struct is loaded from a configuration file at startup,
/// and modified through the GUI.
/// It is then passed to the processing thread to start processing packets.
#[derive(Default, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Settings for input stream
    pub input_settings:  StreamSettings,

    /// Selection of which type of stream to use for input
    pub input_selection:  StreamOption,

    /// Settings for ouput stream
    pub output_settings: StreamSettings,

    /// Selection of which type of stream to use for output
    pub output_selection: StreamOption,

    pub allowed_apids: Option<Vec<u16>>,

    /// GUI theme for IMGUI
    pub theme: GuiTheme,

    /// The packet size for processing- either CCSDS or fixed size
    pub packet_size: PacketSize,

    /// Is the CCSDS header little endian. This is a violation of the standard,
    /// but may be encountered in some systems.
    pub little_endian_ccsds: bool,

    /// The frame settings describe any fixed headers before or after the CCSDS headers.
    pub frame_settings: FrameSettings,

    /// The maximum number of bytes in a packet. This is used to filter out malformed packets
    /// when the maximum length is known beforehand.
    pub max_length_bytes: i32,

    /// The timestamp settings describe how to throttle/delay/replay packets.
    pub timestamp_setting: TimestampSetting,

    /// The timestamp definition describes the location and format of the packet's timestamp.
    /// This must be in the form of a seconds and subseconds field each of 1/2/4 bytes and with
    /// aubseconds of a given resolution.
    pub timestamp_def: TimestampDef,

    /// Start processing on application startup, rather then waiting for the user to click on the
    /// start button.
    #[serde(default)]
    pub auto_start: bool,
}

/// The frame settings describe an enclosing packet header wrapping the CCSDS packets with a fixed
/// number of bytes. There are options to remove or to keep the header/footer in case we want to
/// strip it before forwarding packets, or keep it when forwarding packets.
#[derive(Default, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct FrameSettings {
    pub prefix_bytes: i32,
    pub keep_prefix: bool,
    pub postfix_bytes: i32,
    pub keep_postfix: bool,
}

/* Packet Data */
/// The full set of packet history used when displaying
/// a summary of what packets have been received.
#[derive(Default, PartialEq, Debug, Clone)]
pub struct ProcessingStats {
    pub packet_history: HashMap<Apid, PacketStats>,
    pub packets_per_second: usize,
    pub bytes_per_second: usize,
}

#[derive(PartialEq, Clone, Eq, Debug)]
/// A PacketStats is a set of statistics about a particular
/// APID.
pub struct PacketStats {
    /// The APID of the packet
    pub apid: Apid,

    /// The number of packets received of the APID
    pub packet_count: u16,

    /// The number of bytes received of the APID
    pub byte_count: u32,

    /// The last sequence count read for this APID
    pub last_seq: u16,

    /// The last packet length read for this APID
    pub last_len: u16,

    /// The system time at which the packet was received
    pub recv_time: SystemTime,

    /// The packet itself
    pub bytes: Vec<u8>,
}

impl Default for PacketStats {
    fn default() -> Self {
        PacketStats {
            apid: 0,
            packet_count: 0,
            byte_count: 0,
            last_seq: 0,
            last_len: 0,
            recv_time: SystemTime::now(),
            bytes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A PacketUpdate is provided by the processing thread to
/// the GUI to indicate that a packet was processed.
pub struct PacketUpdate {
    /// The APID of the packet
    pub apid: Apid,

    /// The packet length of the packet
    pub packet_length: u16,

    /// The sequence count of the packet
    pub seq_count: u16,

    /// The system time at which the packet was received
    pub recv_time: SystemTime,

    /// The packet itself
    pub bytes: Vec<u8>,
}

impl PacketStats {
    pub fn update(&mut self, packet_update: PacketUpdate) {
        self.apid = packet_update.apid;
        self.packet_count += 1;
        self.byte_count += packet_update.packet_length as u32;
        self.last_seq = packet_update.seq_count;
        self.last_len = packet_update.packet_length;
        self.recv_time = packet_update.recv_time;
        self.bytes.clear();
        self.bytes.extend(packet_update.bytes);
    }
}

/* Time Settings */
/// The number of bytes in a time field. This is constrained to be
/// a power of 2 from 0 to 2 covering the most common cases.
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum TimeSize {
    ZeroBytes,
    OneByte,
    TwoBytes,
    FourBytes,
}

impl Default for TimeSize {
    fn default() -> Self {
        TimeSize::ZeroBytes
    }
}

impl TimeSize {
    pub fn to_num_bytes(&self) -> usize {
        match self {
            TimeSize::ZeroBytes => 0,
            TimeSize::OneByte   => 1,
            TimeSize::TwoBytes  => 2,
            TimeSize::FourBytes => 4,
        }
    }

    pub fn from_num_bytes(num_bytes: usize) -> Self {
        match num_bytes {
            0 => TimeSize::ZeroBytes,
            1 => TimeSize::OneByte,
            2 => TimeSize::TwoBytes,
            4 => TimeSize::FourBytes,
            _ => TimeSize::ZeroBytes, // not a great default, but probably safe
        }
    }
}

/// Location and definition of packet timestamp.
/// The allowed format is a seconds field followed by
/// a subseconds field, where both fields may be any integer
/// number of bytes.
/// The subseconds field has a resolution (LSB) given as a floating
/// point number.
#[derive(Default, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct TimestampDef {
    /// offset of the timestamp, where 0 is the byte after the 
    /// primary header. This should be 0 to adhere to the CCSDS
    /// time format standard. This field is provided for formats
    /// that do not follow this standard.
    pub offset: i32,

    /// The number of bytes for the seconds field.
    pub num_bytes_seconds: TimeSize,

    /// The number of bytes for the subseconds field.
    pub num_bytes_subseconds: TimeSize,

    /// The resolution of the subseconds field. For example, use 0.001
    /// for millisecond resolution, and 0.000001 for microsecond
    /// resolution, depending on the packet format.
    pub subsecond_resolution: f32,

    /// The endianness of the seconds and subseconds field.
    /// Using a bool makes the GUI code simplier.
    pub is_little_endian: bool,
}

/// The TimestampSetting are the options for how to use time when 
/// processing packets.
/// This allows throttling packet rates, delaying packets (to simulate
/// round trip delays for example), replaying packets at the rate that
/// they were received, or just forwarding packets as fast as possible.
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub enum TimestampSetting {
    /// Forward packets through As Fast As Possible
    Asap,

    /// Send packets at the rate that they were created, based on their
    /// timestamps. This is useful for replaying saved data and approximating
    /// the relative times between packets.
    Replay,

    /// Delay packets once they are recieved. This could be used to simulate
    /// a round trip delay, such as to a production system and back.
    Delay(Duration),

    /// Throttle packets such that they are received at a rate no faster then
    /// a given amount. For example, throttling at 1 second means that packets will
    /// be send out no faster then one per second.
    Throttle(Duration),
}

impl Default for TimestampSetting {
    fn default() -> Self {
        TimestampSetting::Asap
    }
}

/* Messages Generated During Packet Processing */
/// A GuiMessage is a message generated by the processing thread and received
/// by the GUI thread to indicate a change in state or the result of 
/// processing a packet.
#[derive(Debug, PartialEq, Eq)]
pub enum GuiMessage {
    PacketUpdate(PacketUpdate),
    PacketDropped(CcsdsPrimaryHeader),
    Finished,
    Terminate,
    Error(String),
}

/// a ProcessingMsg is a message from the GUI thread to the processing thread
/// commanding a change in state.
#[derive(Debug, PartialEq)]
pub enum ProcessingMsg {
    Start(AppConfig),
    Pause,
    Continue,
    Cancel,
    Terminate,
}

impl ProcessingMsg {
    pub fn name(&self) -> &str {
        match self {
            ProcessingMsg::Start(_) => "Start",
            ProcessingMsg::Pause => "Pause",
            ProcessingMsg::Continue => "Continue",
            ProcessingMsg::Cancel => "Cancel",
            ProcessingMsg::Terminate => "Terminate",
        }
    }
}

/* Packet Processing Thread State */
/// The processing thread is a state machine, so this type gives
/// its possible states.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum ProcessingState {
    /// The processing thread is paused during processing and is waiting for a command
    Paused,
    /// The processing task is processing packets from an input
    Processing,
    /// The processing task is idle and waiting for command to start or close
    Idle,
    /// The processing task is terminating
    Terminating,
}

/// The packet size is used when reading CCSDS- a variable length packet uses the packet length in
/// the CCSDS header, while a fixed size packet assumes we know the packet length beforehand and we
/// do not want to use the packet length.
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum PacketSize {
    /// The packet length is variable and depends on the CCSDS header
    Variable,
    /// The packet length is fixed
    Fixed(u32),
}

impl Default for PacketSize {
    fn default() -> Self {
        PacketSize::Variable
    }
}

impl PacketSize {
    pub fn num_bytes(&self) -> u32 {
        match self {
            PacketSize::Variable => 100,

            PacketSize::Fixed(num) => *num,
        }
    }
}

