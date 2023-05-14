// macOS note: the application this binary is packaged in must have the bluetooth permission

use btleplug::platform::Manager;
use futures::StreamExt;
use std::error::Error;
use std::time::Duration;

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
    Nagios,
}

#[derive(clap::Parser, Debug, Clone, Copy)]
#[command(author, version, about, long_about = None)]
struct Args {
    format: OutputFormat,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let manager = Manager::new().await.unwrap();

    log::info!("discovering BTLE adapters");

    // report basic bluetooth manager errors, retrieve stream of discovered devices
    let mut discovered = aranet::discover_aranet4(&manager).await?;

    log::info!("looking for Aranet4");

    loop {
        // first discovered aranet - may want to impl a timeout
        let Some(first) = discovered.next().await else {
            // no adapters present, unable to wait or discover
            log::error!("Unable to discover devices. No Bluetooth adapters present.");
            break;
        };

        log::info!(
            "Received event from {:?} - {:?} (contains reading: {:?})",
            first.peripheral_id,
            first.manufacturer_data,
            first.current_reading.is_some()
        );
        if let Some(reading) = first.current_reading {
            println!("{}", reading);
        }

        log::debug!("sleeping 60s before attempt receipt of next event...");
        tokio::time::sleep(Duration::from_secs(60)).await;
    }

    Ok(())
}
