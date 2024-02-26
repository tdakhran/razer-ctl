# Reverse engineering
## Sniff a USB port

> Captured on Razer Blade 16 2023

Use [WireShark to capture USB packets](https://wiki.wireshark.org/CaptureSetup/USB#Windows).
To filter razer only packets I used filter `frame.len == 126`.

Also disable all keyboard lightning, it generates a lot of traffic.

An example of single packet: `144	50.302891	host	1.1.0	USBHID	126	001c000000040d0201010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000b00	SET_REPORT Request`

The sniffing process begins with start capture in WireShark, then do actions in Razer Synapse and annotate which action corresponds to which USB packet.

## Process the Data

I dumped some examples
* [wireshark_dump_raw.csv](wireshark_dump_raw.csv) - example data capture from wireshark
* [annotations.csv](annotations.csv) - annotations of actions
* [extract_data_from_capture.py](extract_data_from_capture.py) - script filters out the data and keeps only annotated one

A lot of time is saved thanks to [protocol decoding](https://github.com/openrazer/openrazer/wiki/Reverse-Engineering-USB-Protocol#phase-4---decoding-the-protocol) from [openrazer](https://github.com/openrazer/openrazer).

After running the script, we get the table, that says which command correspods to which action. As you can see, commands have variable number of arguments.

| action                                | cmd   |   argc | arg0   |   arg1 | arg2   |   arg3 |
|---------------------------------------|-------|--------|--------|--------|--------|--------|
| set balanced auto fan mode            | 0d02  |      4 | 01     |     01 | 00     |     00 |
|                                       | 0d02  |      4 | 01     |     02 | 00     |     00 |
| set balanced manual fan mode 3500 rpm | 0d02  |      4 | 01     |     01 | 00     |     01 |
|                                       | 0d02  |      4 | 01     |     02 | 00     |     01 |
|                                       | 0d01  |      3 | 01     |     01 | 23     |        |
|                                       | 0d01  |      3 | 01     |     02 | 23     |        |
| set balanced manual fan mode 2000 rpm | 0d01  |      3 | 01     |     01 | 14     |        |
|                                       | 0d01  |      3 | 01     |     02 | 14     |        |
| set balanced manual fan mode 5000 rpm | 0d01  |      3 | 01     |     01 | 32     |        |
|                                       | 0d01  |      3 | 01     |     02 | 32     |        |
| set balanced auto fan mode            | 0d02  |      4 | 01     |     01 | 00     |     00 |
|                                       | 0d02  |      4 | 01     |     02 | 00     |     00 |
| set silent mode                       | 0d02  |      4 | 01     |     01 | 05     |     00 |
|                                       | 0d02  |      4 | 01     |     02 | 05     |     00 |
| set custom mode, cpu boost, gpu high  | 0d02  |      4 | 01     |     01 | 04     |     00 |
|                                       | 0d02  |      4 | 01     |     02 | 04     |     00 |
|                                       | 0d07  |      3 | 01     |     01 | 03     |        |
|                                       | 0d07  |      3 | 01     |     02 | 02     |        |
| custom mode, cpu high, gpu high       | 0d07  |      3 | 01     |     01 | 02     |        |
| custom mode, cpu middle, gpu high     | 0d07  |      3 | 01     |     01 | 01     |        |
| custom mode, cpu low, gpu high        | 0d07  |      3 | 01     |     01 | 00     |        |
| custom mode, cpu low, gpu middle      | 0d07  |      3 | 01     |     02 | 01     |        |
| custom mode, cpu low, gpu low         | 0d07  |      3 | 01     |     02 | 00     |        |
| custom mode, cpu low, gpu high        | 0d07  |      3 | 01     |     02 | 02     |        |
| custom mode, cpu boost, gpu high      | 0d07  |      3 | 01     |     01 | 03     |        |
| enable max fan speed                  | 070f  |      1 | 02     |        |        |        |
| disable max fan speed                 | 070f  |      1 | 00     |        |        |        |
| custom mode, cpu overclock, gpu high  | 0d07  |      3 | 01     |     01 | 04     |        |
| custom mode, cpu boost, gpu high      | 0d07  |      3 | 01     |     01 | 03     |        |
| battery health, 80%                   | 0712  |      1 | d0     |        |        |        |
| disable battery health                | 0712  |      1 | 50     |        |        |        |
| enable battery health                 | 0712  |      1 | d0     |        |        |        |
| decrease keyboard light               | 0303  |      3 | 01     |     05 | 00     |        |
|                                       | 0303  |      3 | 01     |     05 | 00     |        |
| increase keyboard light               | 0303  |      3 | 01     |     05 | 0f     |        |
| set lid logo static                   | 0302  |      3 | 00     |     04 | 00     |        |
|                                       | 0300  |      3 | 00     |     04 | 01     |        |
| set lid logo breathing                | 0302  |      3 | 00     |     04 | 02     |        |
|                                       | 0300  |      3 | 00     |     04 | 01     |        |
| set lid logo off                      | 0302  |      3 | 00     |     04 | 00     |        |
|                                       | 0300  |      3 | 00     |     04 | 00     |        |
