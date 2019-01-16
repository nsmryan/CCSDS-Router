# CCSDS Router
This application is a tool for moving CCSDS packets (CCSDS Space Packet Protocol) between
files, UDP sockets, and TCP sockets (client or server). It can be run as a GUI program, or
run from a configuration file on the command line.


The main uses cases are to store CCSDS packets coming in from the network, replaying stored
data for testing, and delaying or throttling CCSDS traffic for testing or production. If
you have CCSDS packets from a source, and want to send them to a destination, then this may
be the tool for you!


# Configuration
There are a number of configuration options. These include the source and destination, when to forward a packet,
limits on APID and packet size, and option headers and footers around the packet. These are described below.


## Timing
There are four options for how to forward packets from the input to the output.

* Forward Through- packets are moved from input to output as fast as possible. If the source is a
file, the packets can be send quite quickly.

* Replay- the packets are send to the output at a rate indicated by a timestamp in the packet. The timestamp
must be in the form of a seconds field followed by a subseconds field, with configurable sizes in whole bytes,
and with configurable subsecond resolution. The timestamps will then be used to delay between packets, attempting
to produce the packets according the timestamps.
The use case for this feature is to replay stored data at approximately the timing that it was produced, such as 
for testing a system against stored data.

* Delay- each packet is delayed for a fixed amount. This could be used to test a round-trip delay that would be
seen in production, but not usually seen in testing.

* Throttle- a minimum time between packets is given. This can be used for systems that can only handle data at a
certain rate, such as a command interface that runs on a fixed schedule.

## Framing
The CCSDS packets handled by this application can be framed by a fixed size header and/or footer from another protocol.
There are options to set the length of these sections, and whether to forward the header or footer along with the CCSDS
packet, or to drop it. This can be used when translating packets between interfaces, potentially dropping the headers from
the interface and producing only CCSDS packets.


## Maximum Size
The application allows a maximum packet size configuration item which allows an application-specific maximum packet size. 
The CCSDS standard allows packets with a total size of 65542 including the primary header. However, sometimes we know we will
only receive packets of a certain length, and we can use this to reject packets that are larger then expected as an additional
check on incoming packets.


## APID Filtering
If only certain APIDs should be allowed from input to output, a list of allowed apids can be provided. All other packets will be
dropped.


## Fixed Length Packets
The application allows for packets of a fixed length. In this case, the CCSDS header is not used at all, and blocks of the given
size are forwarded from input to output. Note that this means that if the data stream starts out in the middle of a packet, it will
not be able to resync with the start of a packet and will foward invalid data.

## Little Endian CCSDS Primary Header
The CCSDS standard indicates that the Primary Header should always be Big Endian. However, this application has an option for Little
Endian headers to accomidate this situation for a system that happens to produce packets in this format.


## Configuration
The application makes use of a configuration file in JSON format. All configuration can be set in the GUI,
and saved/loaded. The configuration can be loaded on the command line or through the GUI.


## Logging
The application logs information about its operation and the actions of the operator in a directory called
log, with log files 'ccsds\_router\_log\_YYYYMMDD\_HH\_MM\_SS.log'.


