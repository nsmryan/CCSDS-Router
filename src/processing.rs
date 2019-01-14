use std::time;
use std::default::Default;
use std::sync::mpsc::{Sender, Receiver, RecvTimeoutError};
use std::time::{SystemTime, Duration};

use types::*;
use stream::*;


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

    let mut in_stream  = ReadStream::Null;
    let mut out_stream = WriteStream::Null;

    let mut endianness: Endianness = Endianness::Little;

    let timestamp_setting: TimestampSetting = Default::default();
    let timestamp_def: TimestampDef = Default::default();

    let timeout = Duration::from_millis(0);
    let last_send_time: SystemTime = SystemTime::now();

    loop {
        match state {
            ProcessingState::Idle => {
                in_stream  = ReadStream::Null;
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

                        // get endianness to use
                        if config.little_endian_ccsds {
                            endianness = Endianness::Little;
                        }
                        else {
                            endianness = Endianness::Big;
                        }

                        // open streams
                        match open_input_stream(&config.input_settings, config.input_selection) {
                          Ok(stream) => {
                              in_stream  = stream;

                              match open_output_stream(&config.output_settings, config.output_selection) {
                                Ok(stream) => {
                                    out_stream = stream;
                                    state = ProcessingState::Processing;
                                },

                                Err(err_string) => {
                                    sender.send(GuiMessage::Error(err_string)).unwrap();
                                    sender.send(GuiMessage::Finished).unwrap();
                                },
                              }
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
                let msg_result = receiver.recv().ok();
                match msg_result {
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
                loop {
                    /* Process a Packet */
                    // NOTE need to handle timing out for network reads and still responding to
                    // control messages.
                    // NOTE need to handle reading from files that may grow, and ones that will not
                    match stream_read_packet(&mut in_stream, &mut packet, endianness, packet_size, &frame_settings) {
                        Err(e) => {
                            sender.send(GuiMessage::Error(e)).unwrap();
                            state = ProcessingState::Idle;
                            break;
                        },

                        _ => {},
                    }

                    // determine delay to use from time settings
                    match timestamp_setting {
                        TimestampSetting::Asap => {
                            timeout = Duration::from_millis(0);
                        }

                        TimestampSetting::Replay => {
                            //let timestamp = decode_timestamp(&packet.bytes, &timestamp_def);

                            //match (SystemTime::now() - timestamp).checked_sub(duration) =>
                            //{
                            //    Some(remaining_time) => timeout = remaining_time,

                            //    None => timeout = Duration::from_millis(0);
                            //}
                        },

                        TimestampSetting::Delay(duration) => {
                            timeout = duration;
                        },

                        TimestampSetting::Throttle(duration) => {
                            match (SystemTime::now() - last_send_time).checked_sub(duration) {
                                Some(remaining_time) => timeout = remaining_time,

                                None => timeout = Duration::from_millis(0),
                            }
                        },
                    }

                    /* Check for Control Messages */
                    // NOTE this needs to loop to ensure that we wait the full time, and not
                    // just continue if a message is received.
                    match receiver.recv_timeout(timeout) {
                        Err(RecvTimeoutError::Timeout) => {
                            // timing out means that we are ready to process the next packet,
                            // so this is not an error condition
                        },

                        Ok(ProcessingMsg::Pause) => {
                            state = ProcessingState::Paused;
                            break;
                        },

                        Ok(ProcessingMsg::Cancel) => {
                            state = ProcessingState::Idle;
                            break;
                        },

                        Ok(ProcessingMsg::Terminate) => {
                            state = ProcessingState::Terminating;
                            break;
                        },

                        Ok(msg) => {
                            sender.send(GuiMessage::Error(format!("Unexpected message while processing {}", msg.name()))).unwrap();
                        },

                        Err(RecvTimeoutError::Disconnected) => {
                            // the result is not checked here because we are going to terminate whether
                            // or not it is received.
                            let _ = sender.send(GuiMessage::Error("Message queue error while processing".to_string()));
                            state = ProcessingState::Terminating;
                        },
                    }

                    // check that the packet is within the allowed length. note that this is based
                    // on the CCSDS packet length, ignoring pre or postfix bytes in the frame.
                    if packet.header.packet_length() as usize <= max_length_bytes {
                        stream_send(&mut out_stream, &packet.bytes);

                        /* Report packet to GUI */
                        let packet_update = PacketUpdate { apid: packet.header.control.apid(),
                                                           packet_length: packet.bytes.len() as u16,
                                                           seq_count: packet.header.sequence.sequence_count(),
                                                         };

                        last_send_time = SystemTime::now();

                        sender.send(GuiMessage::PacketUpdate(packet_update)).unwrap();
                    } else {
                        sender.send(GuiMessage::Error(format!("Unexpected message length {} for APID {}. Packet Dropped",
                                                              packet.bytes.len(),
                                                              packet.header.control.apid()))).unwrap();
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
