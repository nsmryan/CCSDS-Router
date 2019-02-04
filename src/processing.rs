use std::default::Default;
use std::sync::mpsc::{Sender, Receiver, RecvTimeoutError, sync_channel};
use std::time::{SystemTime, Duration};
use std::io::Cursor;

use bytes::{Buf};

use ccsds_primary_header::*;

use types::*;
use stream::*;


enum PacketMsg {
    StreamOpenError,
    ReadError(String),
    Packet(Packet, SystemTime),
    PacketDropped(CcsdsPrimaryHeader),
}

// TODO there are a lot of settings here. maybe this is better off in the processing
// thread, or maybe its a good place to simplify things by creating an interface.
fn input_stream_thread(packet_sender: SyncSender<PacketMsg>,
                       read_stream_settings: StreamSettings,
                       input_selection: StreamOption,
                       frame_settings: FrameSettings,
                       endianness: Endianness,
                       packet_size: PacketSize) {
    match open_input_stream(&read_stream_settings, input_selection) {
        Ok(in_stream) => {
            loop {
                let mut packet: Packet
                    = Packet { header: Default::default(),
                               bytes: Vec::with_capacity(4096),
                };

                // NOTE need to handle timing out for network reads and still responding to
                // control messages.
                // NOTE need to handle reading from files that may grow, and ones that will not
                // NOTE have a way to signal nominal end of stream for files, and report back
                // differently
                match stream_read_packet(&mut in_stream, &mut packet, endianness, packet_size, &frame_settings) {
                    Err(e) => {
                        packet_sender.send(PacketMsg::ReadError(e));
                        break;
                    },

                    _ => {
                        let recv_time = SystemTime::now();

                        packet_sender.send(PacketMsg::PacketRead( packet, recv_time ));
                    },
                }
            }
        },

        Err(e) => {
            packet_sender.send(PacketMsg::StreamOpenError);
        }
    }
}

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
            match timestamp_def.is_little_endian {
                false => num_secs = cursor.get_u16_be() as u64,
                true => num_secs = cursor.get_u16_le() as u64,
            }
        },

        TimeSize::FourBytes => {
            match timestamp_def.is_little_endian {
                false => num_secs = cursor.get_u32_be() as u64,
                true => num_secs = cursor.get_u32_le() as u64,
            }
        },
    }

    match timestamp_def.num_bytes_subseconds {
        TimeSize::ZeroBytes => num_subsecs = 0,

        TimeSize::OneByte => num_subsecs = cursor.get_u8() as u64,

        TimeSize::TwoBytes => {
            match timestamp_def.is_little_endian {
                false => num_subsecs = cursor.get_u16_be() as u64,
                true => num_subsecs = cursor.get_u16_le() as u64,
            }
        },

        TimeSize::FourBytes => {
            match timestamp_def.is_little_endian {
                false => num_subsecs = cursor.get_u32_be() as u64,
                true => num_subsecs = cursor.get_u32_le() as u64,
            }
        },
    }

    let subseconds = num_subsecs as f32 * timestamp_def.subsecond_resolution;
    timestamp = Duration::from_secs(num_secs) +
                Duration::from_nanos((1_000_000_000.0 * subseconds.fract()) as u64);

    timestamp
}

/* Packet Processing Thread */
pub fn process_thread(sender: Sender<GuiMessage>, receiver: Receiver<ProcessingMsg>) {
    let mut state: ProcessingState = ProcessingState::Idle;
  
    let mut packet: Packet
        = Packet { header: Default::default(),
                   bytes: Vec::with_capacity(4096),
    };

    let mut max_length_bytes: usize = 0;
    let mut packet_size: PacketSize = Default::default();
    let mut frame_settings: FrameSettings = Default::default();

    let mut out_stream = WriteStream::Null;

    let mut endianness: Endianness = Endianness::Little;

    let mut timestamp_setting: TimestampSetting = Default::default();
    let mut timestamp_def: TimestampDef = Default::default();

    let mut timeout: Duration;
    let mut last_send_time: SystemTime = SystemTime::now();

    let mut allowed_apids = None;

    let mut system_to_packet_time: Option<SystemTime> = None;

    let mut packet_sender: SyncSender<Packet>;
    let mut packet_receiver: SyncSender<Packet>;
    let mut input_thread;

    'state_loop: loop {
        match state {
            ProcessingState::Idle => {
                out_stream = WriteStream::Null;

                let msg_result = receiver.recv().ok();
                match msg_result {
                    // Start processing from a given set of configuration settings
                    Some(ProcessingMsg::Start(config)) => {
                        max_length_bytes = config.max_length_bytes as usize;
                        packet_size = config.packet_size;
                        frame_settings = config.frame_settings;
                        timestamp_setting = config.timestamp_setting;
                        timestamp_def = config.timestamp_def;
                        allowed_apids = config.allowed_apids.clone();

                        // get endianness to use
                        if config.little_endian_ccsds {
                            endianness = Endianness::Little;
                        }
                        else {
                            endianness = Endianness::Big;
                        }

                        // open streams
                        match open_output_stream(&config.output_settings, config.output_selection) {
                          Ok(stream) => {
                              out_stream = stream;

                              // spawn off a thread for reading the input stream
                              // TODO make this a config option for depth
                              (packet_sender, packet_receiver) = sync_channel(100);
                              input_stream_thread = thread::spawn(move || {
                                  input_stream_thread(packet_sender);
                              });

                              state = ProcessingState::Processing;
                          },

                          Err(err_string) => {
                              sender.send(GuiMessage::Error(err_string)).unwrap();
                              sender.send(GuiMessage::Finished).unwrap();
                          },
                        }
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
                system_to_packet_time = None;

                while state == ProcessingState::Processing {
                    /* Process a Packet */
                    let packet_msg = packet_receiver.recv_timeout(0);


                    match packet_msg {
                        PacketMsg::Packet(packet, recv_time) => {
                            // determine delay to use from time settings
                            match timestamp_setting {
                                TimestampSetting::Asap => {
                                    timeout = Duration::from_secs(0);
                                }

                                TimestampSetting::Replay => {
                                   let timestamp = decode_timestamp(&packet.bytes, &timestamp_def);

                                    match system_to_packet_time {
                                        None => {
                                            system_to_packet_time = Some(SystemTime::now() - timestamp);
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

                                TimestampSetting::Delay(duration) => {
                                    // NOTE this puts a delay between each packet, rather then delaying
                                    // a stream of packets. the second design is more correct, but
                                    // would require a separate streaming thread to collect packets for
                                    // us to read out in the processing thread.
                                    timeout = duration;
                                },

                                TimestampSetting::Throttle(duration) => {
                                    // NOTE we unwrap here assuming that system time does not go backwards.
                                    // this assumption is not always true, and we should handle this more
                                    // gracefully.
                                    match duration.checked_sub(last_send_time.elapsed().unwrap()) {
                                        Some(remaining_time) => timeout = remaining_time,

                                        None => timeout = Duration::from_millis(0),
                                    }
                                },
                            }

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

                            stream_send(&mut out_stream, &packet.bytes);

                            /* Report packet to GUI */
                            let mut packet_update = PacketUpdate { apid: packet.header.control.apid(),
                                                                   packet_length: packet.bytes.len() as u16,
                                                                   seq_count: packet.header.sequence.sequence_count(),
                                                                   recv_time: recv_time,
                                                                   bytes: Vec::new(),
                                                                 };

                            packet_update.bytes.extend(packet.bytes.clone());

                            last_send_time = SystemTime::now();

                            sender.send(GuiMessage::PacketUpdate(packet_update)).unwrap();
                        }

                        PacketMsg::PacketDropped(header) => {
                                sender.send(GuiMessage::PacketDropped(header)).unwrap();
                        } 

                        PacketMsg::ReadError(e) => {
                                sender.send(GuiMessage::Error(e)).unwrap();
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
