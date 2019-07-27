use std::default::Default;
use std::sync::mpsc::{SyncSender, Sender, Receiver, RecvTimeoutError, sync_channel};
use std::time::{SystemTime, Duration};
use std::io::Cursor;
use std::thread;
use std::cmp::min;

use bytes::{Buf};
use byteorder::{LittleEndian};

use backplane::*;

use ccsds_primary_header::primary_header::*;
use ccsds_primary_header::parser::{CcsdsParser, CcsdsParserConfig};

use types::*;


#[derive(Debug, Clone)]
enum PacketMsg {
    StreamOpenError,
    ReadError(String),
    Packet(Packet, SystemTime),
    PacketDropped(CcsdsPrimaryHeader),
    StreamParseError,
    StreamEnd,
}

#[derive(Debug, Clone)]
struct TimeState {
  timestamp_setting: TimestampSetting,
  timestamp_def: TimestampDef,
  system_to_packet_time: Option<SystemTime>,
  last_send_time: SystemTime,
}

/// The packet structure contains the data for a packet, as well as the primary header
#[derive(Debug, Clone)]
pub struct Packet {
    pub header: CcsdsPrimaryHeader,
    pub bytes:  Vec<u8>,
}


fn input_stream_thread(packet_sender: SyncSender<PacketMsg>,
                       read_stream_settings: StreamSettings,
                       input_selection: StreamOption,
                       ccsds_parser_config: CcsdsParserConfig) {
    match read_stream_settings.open_input(&input_selection) {
        Ok(ref mut in_stream) => {
            let mut ccsds_parser = CcsdsParser::with_config(ccsds_parser_config.clone());
            ccsds_parser.bytes.reserve(4096);

            'processing_loop: loop {
                // NOTE need to handle timing out for network reads and still responding to
                // control messages.
                // NOTE need to handle reading from files that may grow, and ones that will not
                // NOTE have a way to signal nominal end of stream for files, and report back
                // differently
                // NOTE magic number 4096 is used.
                let current_num_bytes = ccsds_parser.bytes.len();
                let num_bytes_avail = ccsds_parser.bytes.capacity();
                match in_stream.stream_read(&mut ccsds_parser.bytes, num_bytes_avail - current_num_bytes) {
                    Err(e) => {
                        packet_sender.send(PacketMsg::ReadError(e)).unwrap();
                        break;
                    },

                    _ => {
                        // loop, reading all new packets and sending them along.
                        // if there are no new packets, go back to reading the stream for bytes
                        let mut any_packets = false;
                        while let Some(packet_bytes) = ccsds_parser.pull_packet() {
                            let recv_time = SystemTime::now();

                            let mut packet: Packet
                                = Packet { header: Default::default(),
                                           bytes: Vec::with_capacity(packet_bytes.len()),
                            };

                            let bytes = packet_bytes.freeze();
                            if ccsds_parser_config.little_endian_header {
                                let little_header: PrimaryHeader<LittleEndian> = PrimaryHeader::from_slice(&bytes).unwrap();
                                packet.header = little_header.to_big_endian();
                            } else {
                                packet.header = CcsdsPrimaryHeader::from_slice(&bytes).unwrap();
                            }
                            packet.bytes.extend(bytes);

                            packet_sender.send(PacketMsg::Packet(packet, recv_time)).unwrap();

                            any_packets = true;
                        }

                        // if we processed a series of packets, reset the remaining data to the
                        // start of a new parser.
                        if any_packets {
                            let remaining_bytes = ccsds_parser.bytes.freeze();
                            ccsds_parser = CcsdsParser::with_config(ccsds_parser_config.clone());
                            ccsds_parser.bytes.reserve(4096);
                            ccsds_parser.bytes.extend(remaining_bytes);
                        } else if ccsds_parser.bytes.capacity() == ccsds_parser.bytes.len() {
                            // attempt to extend the capacity to accomidate a larger packet.
                            // if this doesn't work, exit.
                            let max_packet_bytes = CCSDS_MAX_LENGTH +
                                                   ccsds_parser.config.num_header_bytes +
                                                   ccsds_parser.config.num_footer_bytes;
                            if ccsds_parser.bytes.len() < max_packet_bytes as usize {
                                let new_capacity = min(max_packet_bytes, (ccsds_parser.bytes.capacity() * 2) as u32);
                                ccsds_parser.bytes.reserve(new_capacity as usize);
                            } else {
                                // NOTE this situation should not happen. The CCSDS parser should
                                // advance over bytes that do not contain a header, and we have
                                // grown the buffer large enough for the largest packet.
                                packet_sender.send(PacketMsg::StreamParseError).unwrap();
                                break 'processing_loop;
                            }
                        }
                    },
                }
            }
        },

        Err(_) => {
            packet_sender.send(PacketMsg::StreamOpenError).unwrap();
        }
    }

    packet_sender.send(PacketMsg::StreamEnd).unwrap();
}

// Decode a timestamp from a vector of bytes into a Duration
// The TimestampDef describes the layout of the timestamp
fn decode_timestamp(bytes: &Vec<u8>, timestamp_def: &TimestampDef) -> Duration {
    let timestamp: Duration;

    let num_secs: u64;
    let num_subsecs: u64;

    let time_start_byte = CCSDS_PRI_HEADER_SIZE_BYTES as usize + timestamp_def.offset as usize;

    let time_length_bytes = timestamp_def.num_bytes_seconds.to_num_bytes() +
                            timestamp_def.num_bytes_subseconds.to_num_bytes();

    let last_byte_offset = time_start_byte + time_length_bytes as usize;

    // make sure there is space in the packet for the timestamp
    if last_byte_offset as usize > bytes.len() {
        return Duration::from_millis(0);
    }

    let timestamp_slice = &bytes[time_start_byte..last_byte_offset];
    let mut cursor = Cursor::new(timestamp_slice);

    match timestamp_def.num_bytes_seconds {
        TimeSize::ZeroBytes => num_secs = 0,

        TimeSize::OneByte => num_secs = cursor.get_u8() as u64,

        TimeSize::TwoBytes => {
            if timestamp_def.is_little_endian {
                num_secs = cursor.get_u16_le() as u64;
            } else {
                num_secs = cursor.get_u16_be() as u64;
            }
        },

        TimeSize::FourBytes => {
            if timestamp_def.is_little_endian {
                num_secs = cursor.get_u32_le() as u64;
            } else {
                num_secs = cursor.get_u32_be() as u64;
            }
        },
    }

    match timestamp_def.num_bytes_subseconds {
        TimeSize::ZeroBytes => num_subsecs = 0,

        TimeSize::OneByte => num_subsecs = cursor.get_u8() as u64,

        TimeSize::TwoBytes => {
            if timestamp_def.is_little_endian {
                num_subsecs = cursor.get_u16_le() as u64;
            } else {
                num_subsecs = cursor.get_u16_be() as u64;
            }
        },

        TimeSize::FourBytes => {
            if timestamp_def.is_little_endian {
                num_subsecs = cursor.get_u32_le() as u64;
            } else {
                num_subsecs = cursor.get_u32_be() as u64;
            }
        },
    }

    let subseconds = num_subsecs as f32 * timestamp_def.subsecond_resolution;
    timestamp = Duration::from_secs(num_secs) +
                Duration::from_nanos((1_000_000_000.0 * subseconds.fract()) as u64);

    timestamp
}

// Determine the timeout we can wait before we need to act again
fn determine_timeout(time_state: &mut TimeState,
                     packet: &Packet) -> Duration {
    let timeout: Duration;

    match time_state.timestamp_setting {
        // Process as fast as possible
        TimestampSetting::Asap => {
            timeout = Duration::from_secs(0);
        }

        // Replaying packets- use the packet's timestamp as an offset
        TimestampSetting::Replay => {
           let timestamp = decode_timestamp(&packet.bytes, &time_state.timestamp_def);

            match time_state.system_to_packet_time {
                None => {
                    time_state.system_to_packet_time = Some(SystemTime::now() - timestamp);
                    timeout = Duration::from_secs(0);
                },

                Some(time_offset) =>
                {
                    let timestamp_sys_time = time_offset + timestamp;

                    match timestamp_sys_time.duration_since(SystemTime::now()) {
                        Ok(remaining_time) => timeout = remaining_time,

                        _ => timeout = Duration::from_secs(0),
                    }
                },
            }
        },

        // delay for a fixed duration
        TimestampSetting::Delay(duration) => {
            timeout = duration;
        },

        // Throttle packet processing to a fixed rate
        // This is different from Delay in that it only delays if necessary to
        // space out packets.
        TimestampSetting::Throttle(duration) => {
            match duration.checked_sub(time_state.last_send_time.elapsed().unwrap()) {
                Some(remaining_time) => timeout = remaining_time,

                None => timeout = Duration::from_millis(0),
            }
        },
    }

    timeout
}

fn start_input_thread(app_config: AppConfig, sender: SyncSender<PacketMsg>) {
    let mut ccsds_parser_config: CcsdsParserConfig = CcsdsParserConfig::new();

    ccsds_parser_config.allowed_apids = app_config.allowed_input_apids.clone();

    match app_config.packet_size {
        PacketSize::Variable =>
            ccsds_parser_config.max_packet_length = None,

        PacketSize::Fixed(num_bytes) =>
            ccsds_parser_config.max_packet_length = Some(num_bytes),
    }
    ccsds_parser_config.num_header_bytes = app_config.frame_settings.prefix_bytes as u32;
    ccsds_parser_config.keep_header = app_config.frame_settings.keep_prefix;
    ccsds_parser_config.keep_sync = app_config.frame_settings.keep_prefix;

    ccsds_parser_config.num_footer_bytes = app_config.frame_settings.postfix_bytes as u32;
    ccsds_parser_config.keep_footer = app_config.frame_settings.keep_postfix;

    ccsds_parser_config.little_endian_header = app_config.little_endian_ccsds;

    let _input_stream_thread = thread::spawn(move || {
        input_stream_thread(sender,
                            app_config.input_settings,
                            app_config.input_selection,
                            ccsds_parser_config);
    });
}

/* Packet Processing Thread */
pub fn process_thread(sender: Sender<GuiMessage>, receiver: Receiver<ProcessingMsg>) {
    let mut state: ProcessingState = ProcessingState::Idle;
  
    let mut output_streams = vec!();

    let mut timeout: Duration;

    let (_, mut packet_receiver) = sync_channel(100);

    let mut app_config: AppConfig = Default::default();

    'state_loop: loop {
        match state {
            ProcessingState::Idle => {
                output_streams = vec!();

                let msg_result = receiver.recv().ok();
                match msg_result {
                    // Start processing from a given set of configuration settings
                    Some(ProcessingMsg::Start(config)) => {
                        app_config = config;

                        // open streams
                        for index in 0..app_config.output_settings.len() {
                            let output_stream = app_config.output_settings[index]
                                                .open_output(&app_config.output_selection[index]);
                                                                   
                            match output_stream {
                                Ok(stream) => {
                                    output_streams.push(stream)
                                },

                                Err(err_string) => {
                                    sender.send(GuiMessage::Error(err_string)).unwrap();
                                    sender.send(GuiMessage::Finished).unwrap();
                                    state = ProcessingState::Idle;
                                    output_streams = vec!();
                                    continue 'state_loop;
                                },
                             }
                        }

                        // spawn off a thread for reading the input stream
                        // TODO make this a config option for depth
                        let (sender, receiver) = sync_channel(100);
                        packet_receiver = receiver;

                        start_input_thread(app_config.clone(), sender);
                        state = ProcessingState::Processing;
                    },

                    Some(ProcessingMsg::Terminate) => {
                        state = ProcessingState::Terminating;
                    },

                    Some(msg) => {
                        sender.send(GuiMessage::Error(format!("Unexpected message while waiting to process {}", msg.name()))).unwrap();
                    }

                    None => {
                        // the result is not checked here because we are going to terminate whether
                        // or not it is received.
                        sender.send(GuiMessage::Error("Message queue error while idle".to_string())).unwrap();
                        state = ProcessingState::Terminating;
                    },
                }
            },

            ProcessingState::Paused => {
                match receiver.recv().ok() {
                    Some(ProcessingMsg::Continue) => {
                        state = ProcessingState::Processing;
                    },

                    Some(ProcessingMsg::Cancel) => {
                        state = ProcessingState::Idle;
                    },

                    Some(ProcessingMsg::Terminate) => {
                        state = ProcessingState::Terminating;
                    },

                    Some(msg) => {
                        sender.send(GuiMessage::Error(format!("Unexpected message while paused {}", msg.name()))).unwrap();
                    }

                    None => {
                        // the result is not checked here because we are going to terminate whether
                        // or not it is received.
                        sender.send(GuiMessage::Error("Message queue error while paused".to_string())).unwrap();
                        state = ProcessingState::Terminating;
                    },
                }
            },

            ProcessingState::Processing => {
                let mut time_state = TimeState{
                                 timestamp_setting: app_config.timestamp_setting.clone(),
                                 timestamp_def: app_config.timestamp_def.clone(),
                                 system_to_packet_time: None,
                                 last_send_time: SystemTime::now(),
                };


                while state == ProcessingState::Processing {
                    /* Process a Packet */
                    let packet_msg = packet_receiver.recv();

                    match packet_msg {
                        Ok(PacketMsg::Packet(packet, recv_time)) => {
                            // determine delay to use from time settings
                            timeout = determine_timeout(&mut time_state, &packet);

                            /* Check for Control Messages */
                            let time_to_send = SystemTime::now() + timeout;

                            // process at least one message. continue to process messages until we have
                            // reached the timeout period for processing this packet.
                            let mut processed_at_least_once = false;
                            let mut remaining_timeout = timeout;
                            while !processed_at_least_once || SystemTime::now() < time_to_send {
                                match receiver.recv_timeout(remaining_timeout) {
                                    Err(RecvTimeoutError::Timeout) => {
                                        // timing out means that we are ready to process the next packet,
                                        // so this is not an error condition
                                    },

                                    Ok(ProcessingMsg::Pause) => {
                                        // we will pause after processing this packet
                                        state = ProcessingState::Paused;
                                    },

                                    Ok(ProcessingMsg::Cancel) => {
                                        state = ProcessingState::Idle;
                                        continue 'state_loop;
                                    },

                                    Ok(ProcessingMsg::Terminate) => {
                                        state = ProcessingState::Terminating;
                                        continue 'state_loop;
                                    },

                                    Ok(msg) => {
                                        sender.send(GuiMessage::Error(format!("Unexpected message while processing {}", msg.name()))).unwrap();
                                    },

                                    Err(RecvTimeoutError::Disconnected) => {
                                        // the result is not checked here because we are going to terminate whether
                                        // or not it is received.
                                        let _ = sender.send(GuiMessage::Error("Message queue error while processing".to_string()));
                                        state = ProcessingState::Terminating;
                                        continue 'state_loop;
                                    },
                                }

                                processed_at_least_once = true;

                                // the remaining timeout is the duration from now to the send time. if the
                                // send time is in the past, use a duration of 0.
                                remaining_timeout = SystemTime::now().duration_since(time_to_send).unwrap_or(Duration::from_secs(0));
                            }

                            // send output to each stream, filtering by allowed apids
                            for index in 0..output_streams.len() {
                                let apid_allowed;

                                match app_config.allowed_output_apids[index] {
                                    Some(ref apids) => {
                                        apid_allowed = apids.contains(&packet.header.control.apid());
                                    },

                                    None => apid_allowed = true,
                                }
                                
                                if apid_allowed {
                                    output_streams[index].stream_send(&packet.bytes).unwrap();
                                }
                            }

                            /* Report packet to GUI */
                            let mut packet_update = PacketUpdate { apid: packet.header.control.apid(),
                                                                   packet_length: packet.bytes.len() as u16,
                                                                   seq_count: packet.header.sequence.sequence_count(),
                                                                   recv_time: recv_time,
                                                                   bytes: Vec::new(),
                                                                 };

                            packet_update.bytes.extend(packet.bytes.clone());

                            time_state.last_send_time = SystemTime::now();

                            sender.send(GuiMessage::PacketUpdate(packet_update)).unwrap();
                        }

                        Ok(PacketMsg::PacketDropped(header)) => {
                                sender.send(GuiMessage::PacketDropped(header)).unwrap();
                        } 

                        Ok(PacketMsg::StreamParseError) => {
                            // NOTE this could be presented as an error, rather than panicing
                            panic!("There was a unrecoverable parsing error while streaming data!");
                        } 

                        Ok(PacketMsg::ReadError(e)) => {
                                sender.send(GuiMessage::Error(e)).unwrap();
                        }

                        Ok(PacketMsg::StreamOpenError) => {
                            // NOTE this could be presented as an error, rather than panicing
                            panic!("The packet stream could not be opened!")
                        }

                        Ok(PacketMsg::StreamEnd) => {
                            state = ProcessingState::Idle;
                        }

                        Err(e) => {
                            // NOTE this could be presented as an error, rather than panicing
                            panic!(e)
                        }
                    }
                }

                sender.send(GuiMessage::Finished).unwrap();
            },

            ProcessingState::Terminating => {
                break;
            },
        } // match state
    } // loop

    // the result is not inspected here- we are going to exit whether or not our message is received.
    let _ = sender.send(GuiMessage::Terminate);
}

