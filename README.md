# Aranet4 CLI and HTTP Server

The goal is to perform most data reporting functions of the mobile app, and expose that functionality through a CLI and web interfaces.

## Library

Provides:
* Bluetooth UUIDs for Aranet specific services
* Serde-compatible structs for the data from the probe
* Async Rust bindings around a discovered Aranet4 Bluetooth device
* Waiting for an advertisement from all bluetooth adapters

## CLI

A simple CGI-capable binary that allows one to fetch readings and output them in Text, JSON, or Nagios formats.

```
Usage: aranet [OPTIONS]

Options:
  -f, --format <FORMAT>      The output format. If --forever is passed with --format=json,
                             then it will be one JSON object per line [default: text]
                             [possible values: text, json, nagios]
  -a, --active               Request a sample actively, instead of waiting for a manufacturer advertisement
  -r, --repeat               Keep listening and outputting samples instead of exiting after the first sample.
                             Note that --format=nagios will ignore this option, and only output once
  -i, --interval <INTERVAL>  If --repeat is passed, the wait interval between listening for samples. If 0, then
                             the interval from the device is used. Passing -1 will disable waiting.
  -d, --device <DEVICE>      Listen for a specific Aranet4 device, rather than the first available
  -h, --help                 Print help
  -V, --version              Print version
```

```
$ cargo run
    Finished dev [unoptimized + debuginfo] target(s) in 0.34s
     Running `target/debug/aranet`
Measurement Age: 73/300s
Battery: 73%
CO2: 867 PPM
CO2 Status: Green
Temperature: 82.6°F (28.1°C)
Rel. Humidity: 43%
Pressure: 0.984 atm (997 hPa)
```

Takes about 1-3 seconds per run (and consequently, as a CGI script).
This time is almost entirely waiting for the next advertisement.

```
$ hyperfine --runs=60 .\target\debug\aranet.exe
Benchmark #1: .\target\debug\aranet.exe
  Time (mean ± σ):      1.249 s ±  0.990 s    [User: 4.7 ms, System: 8.0 ms]
  Range (min … max):    0.130 s …  4.582 s    60 runs
```

Can also simply be used to test the bluetooth stack:
```sh
RUST_LOG=aranet=trace # environment variable to enable trace debugging
aranet --repeat --interval -1.0 # look for advertisements forever
# has the side effect of debug-printing all events
# from every bluetooth adapter, until killed
```

## Examples

### [Dump Advertisements](examples/dump_advertisements.rs)
Read the first advertisement for an Aranet4 device on any bluetooth interface, connected or not, and print the results.

# Building/Usage

## Windows

Should 'just work' - `cargo run`

## Debian/Ubuntu

**Assumes a functional bluetooth stack. Seems finicky on RPi lately.**

May need to install `libdbus-1-dev` and `pkg-config` before running `cargo run`

## MacOS

Please see a [note from the bluetooth library on application permissions](https://github.com/deviceplug/btleplug#macos).

# Integration with other tools

A roadmap for usage into other platforms.

## LibreNMS

If CGI environment variable `GATEWAY_INTERFACE` exists and feature `cgi_detection` is enabled, then it will consider the request to be from CGI. It notably formats output correctly and accepts a `format` query string of (`text`, `nagios`, or `json`) depending on enabled features. Similarly checks the HTTP Accept header of `text/plain` (text format), and `application/json`.

A light webserver may eventually be included for basic control via REST api.

For now, the author uses this workflow:

* An RPi4 Raspbian server
    * LibreNMS/Nginx
    * Connects to network over copper with a static IP
    * Bluetooth broke after initial software updates for some reason
* An RPi3B+ "server"
    * Dedicated to [DakBoard](https://dakboard.com/) as it's primary application
    * Connects to network over WiFi with a static IP
    * Bluetooth works on it
    * This repo cloned to it, with the built binary
    * there's no reason to use this extra server except for missing bluetooth
* An Aranet4 environmental probe
    * "Smart Home integration" enabled
    * Display units in the app are irrelavent for this application
    * Tested with firmwave v1.2.0

### On the RPi3B+ (Rasbpian 10)

Clone this repo, build the binary. With Python3 installed, put a copy of (or symlink to) the built binary into a new folder, with a 'cgi-bin' folder inside.

```sh
sudo apt-get install libdbus-1-dev pkg-config tmux
git clone https://github.com/chrismooredev/aranet-rs
cd aranet-rs
cargo build --release
mkdir -p httpserver_root/cgi-bin
ln -s ../../target/release/aranet httpserver_root/cgi-bin/aranet
cd httpserver_root
tmux new -s httpsrv python3 -m http.server --cgi
# ctrl-b + d to detach
```

These basic instructions could also be adapted for use on a Windows machine. If you can handle forced restarts with updates and a constantly open terminal.

### On the LibreNMS server

Create a new nagios service that just calls curl to the new webserver. The following ensures error conditions are captured semi-appropriately:

> [`/usr/lib/nagios/plugins/check_aranet4`](./check_aranet4)
```sh
#!/bin/bash

WEBSERVER=localhost

exec 3>&1 # capture stdout

# save stderr in variable, letting stdout through
stderr=$(curl --silent --show-error "http://$WEBSERVER:8000/cgi-bin/aranet?format=nagios" 2>&1 1>&3)
status=$?

exec 3>&- # close temp fd

if [ $status -ne 0 ] ; then
        echo "ERROR - $stderr"
        exit 1
fi
```
