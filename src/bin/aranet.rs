// macOS note: the application this binary is packaged in must have the bluetooth permission

use aranet::{CurrentReadingDetailed, ManufacturerData};
use btleplug::api::{CentralEvent};
use btleplug::api::{Central, Manager as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, PeripheralId};
use futures::{future, StreamExt, Stream};
use std::error::Error;
use std::time::Duration;
use std::pin::Pin;

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
    Nagios,
}

#[derive(clap::Parser, Debug, Clone, Copy)]
#[command(author, version, about, long_about = None)]
struct Args {
    format: OutputFormat
}

#[derive(Debug, Clone)]
struct DiscoveredAranet {
    adapter: Adapter,
    peripheral_id: PeripheralId,
    manufacturer_data: ManufacturerData,
    current_reading: Option<CurrentReadingDetailed>,
}

/// Attempt to locate an Aranet4 device, by finding a device that advertises manufacturer data with the correct ID
async fn find_aranet4(manager: &Manager) -> btleplug::Result<Pin<Box<impl Stream<Item = DiscoveredAranet>>>> {

    let adapters = manager.adapters().await?;
    log::debug!("Found {} BTLE adapters", adapters.len());
    let mut event_streams = Vec::with_capacity(adapters.len());

    for (adapter_idx, adapter) in adapters.into_iter().enumerate() {
        log::debug!("BTLE Adapter#{} - Found {:?}", adapter_idx, adapter);
        adapter.start_scan(ScanFilter { services: vec![aranet::uuids::AR4_SERVICE] }).await?;
        log::debug!("BTLE Adapter#{} - Started scanning", adapter_idx);
        let events = adapter.events().await?;
        log::debug!("BTLE Adapter#{} - Listening", adapter_idx);
        let inspected = events.inspect(move |ce| {
            log::trace!("BTLE Adapter#{} - Event {:?}", adapter_idx, ce);
        });
        event_streams.push(inspected.filter_map(move |ce| {
            future::ready(match ce {
                CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data } => {
                    if let Some(data) = manufacturer_data.get(&aranet::uuids::MANUFACTURER_ID) {
                        let raw_manuf = data[..7].try_into()
                            .expect("Aranet4's Manufacturer ID used for advertisement data under 7 bytes!");
                        let manufacturer_data = ManufacturerData::parse(raw_manuf);
                        let current_reading = data[8..21].try_into()
                            .map(CurrentReadingDetailed::parse)
                            .ok();
                        Some(DiscoveredAranet {
                            adapter: adapter.clone(),
                            peripheral_id: id,
                            manufacturer_data,
                            current_reading,
                        })
                    } else {
                        /* unknown manufacturer ID */
                        None
                    }
                },
                _ => {
                    /* other discovery methods may be implemented in the future, for now - just manufacturer data */
                    None
                }
            })
        }));
    }

    log::debug!("listening on {} BTLE adapters", event_streams.len());
    Ok(Box::pin(futures::stream::select_all(event_streams)))
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let manager = Manager::new().await.unwrap();

    log::info!("discovering BTLE adapters");

    // report basic bluetooth manager errors, retrieve stream of discovered devices
    let mut discovered = find_aranet4(&manager).await?;

    log::info!("looking for Aranet4");

    loop {
        // first discovered aranet - may want to impl a timeout
        let Some(first) = discovered.next().await else {
            // no adapters present, unable to wait or discover
            log::error!("Unable to discover devices. No Bluetooth adapters present.");
            break;
        };


        log::info!("Received event from {:?} - {:?} (contains reading: {:?})", first.peripheral_id, first.manufacturer_data, first.current_reading.is_some());
        if let Some(reading) = first.current_reading {
            println!("{}", reading);
        }

        log::debug!("sleeping 60s before attempt receipt of next event...");
        tokio::time::sleep(Duration::from_secs(60)).await;
    }

    Ok(())
}