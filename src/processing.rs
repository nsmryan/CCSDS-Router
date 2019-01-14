use std::time;
use std::default::Default;
use std::sync::mpsc::{Sender, Receiver};

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
                    /* Check for Control Messages */
                    let proc_msg = receiver.recv_timeout(time::Duration::from_millis(0)).ok();
                    match proc_msg {
                        Some(ProcessingMsg::Pause) => {
                            state = ProcessingState::Paused;
                            break;
                        },

                        Some(ProcessingMsg::Cancel) => {
                            state = ProcessingState::Idle;
                            break;
                        },

                        Some(ProcessingMsg::Terminate) => {
                            state = ProcessingState::Terminating;
                            break;
                        },

                        Some(msg) => {
                            sender.send(GuiMessage::Error(format!("Unexpected message while processing {}", msg.name()))).unwrap();
                        }

                        None => {
                            // NOTE should check for errors. ignore timeouts, but report others.
                            // the result is not checked here because we are going to terminate whether
                            // or not it is received.
                            //sender.send(GuiMessage::Error("Message queue error while processing".to_string())).unwrap();
                            //state = ProcessingState::Terminating;
                        }
                    }

                    /* Process a Packet */
                    match stream_read_packet(&mut in_stream, &mut packet, endianness, packet_size, &frame_settings) {
                        Err(e) => {
                            sender.send(GuiMessage::Error(e)).unwrap();
                            state = ProcessingState::Idle;
                            break;
                        },

                        _ => {},
                    }

                    if packet.header.packet_length() as usize <= max_length_bytes {
                        stream_send(&mut out_stream, &packet.bytes);

                        /* Report packet to GUI */
                        let packet_update = PacketUpdate { apid: packet.header.control.apid(),
                                                           packet_length: packet.bytes.len() as u16,
                                                           seq_count: packet.header.sequence.sequence_count(),
                                                         };

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
