use std::time::Duration;
use std::default::Default;
use std::collections::HashMap;


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
#[derive(Default, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub input_settings:  StreamSettings,
    pub input_selection:  StreamOption,

    pub output_settings: StreamSettings,
    pub output_selection: StreamOption,

    pub theme: GuiTheme,

    pub packet_size: PacketSize,
    pub little_endian_ccsds: bool,

    pub prefix_bytes: i32,
    pub keep_prefix: bool,
    pub postfix_bytes: i32,
    pub keep_postfix: bool,

    pub max_length_bytes: i32,

    pub timestamp_setting: TimestampSetting,
    pub timestamp_def: TimestampDef,
}

/* Packet Data */
/// The full set of packet history used when displaying
/// a summary of what packets have been received.
pub type PacketHistory = HashMap<Apid, PacketStats>;

#[derive(Default, PartialEq, Copy, Clone, Eq, Debug)]
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
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// A PacketUpdate is provided by the processing thread to
/// the GUI to indicate that a packet was processed.
pub struct PacketUpdate {
    /// The APID of the packet
    pub apid: Apid,

    /// The packet length of the packet
    pub packet_length: u16,

    /// The sequence count of the packet
    pub seq_count: u16,
}

impl PacketStats {
    pub fn update(&mut self, packet_update: PacketUpdate) {
        self.apid = packet_update.apid;
        self.packet_count += 1;
        self.byte_count += packet_update.packet_length as u32;
        self.last_seq = packet_update.seq_count;
    }
}

/* Time Settings */
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
    pub num_bytes_seconds: i32,

    /// The number of bytes for the subseconds field.
    pub num_bytes_subseconds: i32,

    /// The resolution of the subseconds field. For example, use 0.001
    /// for millisecond resolution, and 0.000001 for microsecond
    /// resolution, depending on the packet format.
    pub subsecond_resolution: f32,
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
pub enum ProcessingState {
    Paused,
    Processing,
    Idle,
    Terminating,
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

