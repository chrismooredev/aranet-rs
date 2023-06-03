// macOS note: the application this binary is packaged in must have the bluetooth permission

use btleplug::api::BDAddr;
use btleplug::platform::Manager;
use clap::Parser;
use futures::StreamExt;
#[cfg(feature = "nagiosplugin")]
use nagiosplugin::{Resource, CheckResult, UnitString, RunnerResult, ServiceState, PerfString, Unit};
use std::error::Error;
use std::fmt;
use std::time::Duration;

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OutputFormat {
    Text,
    #[cfg(feature = "serde_json")]
    Json,
    #[cfg(feature = "nagiosplugin")]
    Nagios,
}
impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            OutputFormat::Text => "text",
            #[cfg(feature = "serde_json")]
            OutputFormat::Json => "json",
            #[cfg(feature = "nagiosplugin")]
            OutputFormat::Nagios => "nagios",
        })
    }
}

#[derive(clap::Parser, Debug, Clone, Copy)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The output format.
    #[cfg_attr(feature = "serde_json", doc = "If --forever is passed with --format=json, then it will be one JSON object per line")]
    #[arg(short, long, default_value_t=OutputFormat::Text)]
    format: OutputFormat,
    /// Request a sample actively, instead of waiting for a manufacturer advertisement
    #[arg(short, long)]
    active: bool,
    /// Keep listening and outputting samples instead of exiting after the first sample.
    #[cfg_attr(feature = "nagiosplugin", doc = "Note that --format=nagios will ignore this option, and only output once.")]
    #[arg(short, long)]
    repeat: bool,
    /// If --repeat is passed, the wait interval between listening for samples. If 0, then the interval from the device
    /// is used. Passing -1 will disable waiting.
    #[arg(short, long, allow_hyphen_values = true)]
    interval: Option<f64>,
    /// Listen for a specific Aranet4 device, rather than the first available
    #[arg(short, long)]
    device: Option<BDAddr>,
}

impl Args {
    /// Updates arguments from CGI environment variables, if they exist.
    #[cfg(feature = "cgi_detection")]
    fn update_from_cgi(&mut self) {
        use std::collections::HashMap;

        if std::env::var("GATEWAY_INTERFACE").is_err() {
            log::debug!("no cgi environment detected");
            return;
        }

        if let Ok(querystr) = std::env::var("QUERY_STRING") {
            let kvs: HashMap<_, _> = querystr.split('&')
                .filter_map(|chunk| chunk.split_once('='))
                .collect();
            
            let find_key = |s: &str| kvs.iter().filter_map(|(k,v)| k.eq_ignore_ascii_case(s).then_some(v)).next();

            if let Some(fmt) = find_key("format") {
                if fmt.eq_ignore_ascii_case("text") {
                    self.format = OutputFormat::Text;
                } else if fmt.eq_ignore_ascii_case("nagios") {
                    #[cfg(feature = "nagiosplugin")] {
                        self.format = OutputFormat::Nagios;
                    }
                    #[cfg(not(feature = "nagiosplugin"))] {
                        println!("Content-Type: text/plain");
                        println!();
                        println!("Nagios not supported in this build");
                        std::process::exit(0);
                    }
                } else if fmt.eq_ignore_ascii_case("json") {
                    #[cfg(feature = "serde_json")] {
                        self.format = OutputFormat::Json;
                    }
                    #[cfg(not(feature = "serde_json"))] {
                        println!("Content-Type: application/json");
                        println!();
                        println!("{{ \"status\": \"error\", \"message\": \"JSON not supported in this build\" }}");
                        std::process::exit(0);
                    }
                }
            }

            // if let Some(active) = find_key("active") {
            //     if ! active.is_empty() {
            //         self.active = true;
            //     }
            // }
        }

        if let Ok(accept) = std::env::var("HTTP_ACCEPT") {
            if let Some(mime) = accept.split(",").next() {
                if mime.eq_ignore_ascii_case("text/plain") {
                    self.format = OutputFormat::Text;
                } else if mime.eq_ignore_ascii_case("application/json") {
                    #[cfg(feature = "serde_json")] {
                        self.format = OutputFormat::Json;
                    }
                    #[cfg(not(feature = "serde_json"))] {
                        println!("Content-Type: application/json");
                        println!();
                        println!("{{ \"status\": \"error\", \"message\": \"JSON not supported in this build\" }}");
                        std::process::exit(0);
                    }
                }
            }
        }

        println!("Content-Type: {}; charset=utf-8", match self.format {
            OutputFormat::Text => "text/plain",
            #[cfg(feature = "nagiosplugin")]
            OutputFormat::Nagios => "text/plain",
            #[cfg(feature = "serde_json")]
            OutputFormat::Json => "application/json",
        });
        println!();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let mut args = Args::parse();

    log::debug!("cli arguments: {:?}", args);

    #[cfg(feature = "cgi_detection")] {
        args.update_from_cgi();
        log::debug!("cgi arguments: {:?}", args)
    }

    if args.active {
        todo!("active sample request not yet implemented");
    }

    let manager = Manager::new().await.unwrap();

    log::info!("discovering BTLE adapters");

    // report basic bluetooth manager errors, retrieve stream of discovered devices
    let mut discovered = aranet::discover_aranet4(&manager).await?;

    log::info!("looking for Aranet4");

    loop {
        // first discovered aranet - may want to impl a timeout
        let Some(first) = discovered.next().await else {
            // no adapters present, unable to wait or discover
            let msg = "Unable to discover devices. No Bluetooth adapters present.";
            match args.format {
                OutputFormat::Text => eprintln!("{}", msg),
                #[cfg(feature = "serde_json")]
                OutputFormat::Json => eprintln!(r#"{{"status": "error", "message": {:?}}}"#, msg),
                #[cfg(feature = "nagiosplugin")]
                OutputFormat::Nagios => RunnerResult::Err(ServiceState::Critical, msg).print_and_exit()
            }
            break;
        };

        if let Some(dev) = args.device {
            if format!("{}", first.peripheral_id) != format!("{}", dev) {
                // got the wrong device
                continue;
            }
        }

        match args.format {
            OutputFormat::Text => {
                log::info!(
                    "Received event from {:?} - {:?} (contains reading: {:?})",
                    first.peripheral_id,
                    first.manufacturer_data,
                    first.current_reading.is_some()
                );
                if let Some(reading) = first.current_reading {
                    println!("{}", reading);
                } else {
                    println!("<no sample data included in advertisement>");
                }
            },
            #[cfg(feature = "serde_json")]
            OutputFormat::Json => {
                if ! args.repeat {
                    println!("{}", serde_json::to_string_pretty(&first).expect("unable to serialize advertisement as JSON"));
                } else {
                    println!("{}", serde_json::to_string(&first).expect("unable to serialize advertisement as JSON"));
                }
            },
            #[cfg(feature = "nagiosplugin")]
            OutputFormat::Nagios => {

                let desc = match first.current_reading {
                    None => format!("Advertisement from {}, Firmware {} (Measurement not included)", first.peripheral_id, first.manufacturer_data.version),
                    Some(cr) => format!("Advertisement from {}, Firmware {} (Measurement age {}/{}s)", first.peripheral_id, first.manufacturer_data.version, cr.age, cr.interval),
                };

                let mut res = Resource::new("Aranet4")
                    .with_description(desc)
                    .with_fixed_state(if first.current_reading.is_some() { ServiceState::Ok } else { ServiceState::Warning });
                
                if let Some(r) = first.current_reading {
                    res.push_result(CheckResult::new().with_perf_data(PerfString::new("battery", &((r.battery*100.0) as u8), Unit::Percentage, Some(&30), Some(&10), Some(&0), Some(&100))));
                    res.push_result(CheckResult::new().with_perf_data(PerfString::new("co2_status", &(r.status as u8), Unit::None, Some(&2), Some(&3), Some(&1), Some(&3))));
                    res.push_result(CheckResult::new().with_perf_data(PerfString::new("humidity", &((r.humidity*100.0) as u8), Unit::Percentage, None, None, Some(&0), Some(&100))));
                    if let Some(ppm) = r.co2_ppm {
                        res.push_result(CheckResult::new().with_perf_data(PerfString::new("co2_ppm", &ppm, Unit::Other(UnitString::new("ppm").unwrap()), None, None, Some(&0), None)));
                    }
                    if let Some(f) = r.temperature_f() {
                        res.push_result(CheckResult::new().with_perf_data(PerfString::new("temperature_f", &f, Unit::Other(UnitString::new("F").unwrap()), None, None, Some(&0.0), None)));
                    }
                    if let Some(atm) = r.pressure_atm() {
                        res.push_result(CheckResult::new().with_perf_data(PerfString::new("pressure_atm", &atm, Unit::Other(UnitString::new("atm").unwrap()), None, None, Some(&0.0), None)));
                    }
                }

                RunnerResult::<()>::Ok(res).print_and_exit();
            }
        }

        if ! args.repeat {
            break;
        }

        let interval = match (args.interval, first.current_reading) {
            (Some(i), _) => i,
            (_, Some(r)) => r.interval as f64,
            (_, _) => {
                log::trace!("requested device interval but device did not provide a reading! using 60s default");
                60.0
            },
        };

        if interval != -1.0 {
            log::debug!("sleeping {}s before attempt receipt of next event...", interval);
            tokio::time::sleep(Duration::from_secs_f64(interval)).await;
        }
    }

    Ok(())
}
