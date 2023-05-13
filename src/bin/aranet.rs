// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

use aranet::{CurrentReadingDetailed, ManufacturerData};
use btleplug::api::{Service, CentralEvent, BDAddr};
use btleplug::api::{
    bleuuid::uuid_from_u16, Central, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use futures::{future, TryFutureExt, TryStreamExt, StreamExt};
use rand::{thread_rng, Rng};
use std::borrow::Borrow;
use std::error::Error;
use std::fmt::Display;
use std::time::Duration;
use uuid::Uuid;

const LIGHT_CHARACTERISTIC_UUID: Uuid = uuid_from_u16(0xFFE9);
use tokio::time;

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

fn event_peripheral_id(e: &CentralEvent) -> PeripheralId {
    match e {
        CentralEvent::DeviceConnected(pid) => pid,
        CentralEvent::DeviceDisconnected(pid) => pid,
        CentralEvent::DeviceDiscovered(pid) => pid,
        CentralEvent::DeviceUpdated(pid) => pid,
        CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data } => id,
        CentralEvent::ServiceDataAdvertisement { id, service_data } => id,
        CentralEvent::ServicesAdvertisement { id, services } => id,
    }.clone()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Event {
    ManufData(ManufacturerData, Option<CurrentReadingDetailed>),
}

async fn find_aranet4(manager: &Manager) -> btleplug::Result<Option<Peripheral>> {
    
    let rt = tokio::runtime::Handle::try_current().unwrap();

    let (es, mut er) = tokio::sync::watch::channel(Option::<Event>::None);

    let adapters = manager.adapters().await;
    eprintln!("adapters: {:?}", adapters);

    let data_listener = async move {
        for i in 0..10 {
            if er.changed().await.is_err() { break; }
            eprintln!("[{:?}] Event {}: {:?}", chrono::prelude::Local::now(), i, *er.borrow());
        }
    };

    // loop over bluetooth adapters
    let data_discovery = async move {
        for adapter in manager.adapters().await? {
            // start scanning for devices
            adapter.start_scan(ScanFilter { services: vec![aranet::uuids::AR4_SERVICE] }).await?;

            let mut events = adapter.events().await?;
            while let Some(e) = events.next().await {
                let peripheral_id = event_peripheral_id(&e);
                if peripheral_id.to_string() != "DB:CC:EB:73:3B:2E" {
                    continue;
                }

                // eprintln!("event: {:?}", e);
                match e {
                    CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data } => {
                        if let Some(data) = manufacturer_data.get(&aranet::uuids::MANUFACTURER_ID) {
                            // eprintln!("raw: {:?}, {:?}", data, &data[..7]);
                            let manuf = ManufacturerData::parse(data[..7].try_into().expect("ahjhh"));
                            let sensor = data[8..21].try_into()
                                .map(CurrentReadingDetailed::parse)
                                .ok();
                            // eprintln!("manuf: {:?}", manuf);
                            // eprintln!("sensor: {:?}", sensor);
                            if let Err(_) = es.send(Some(Event::ManufData(manuf, sensor))) {
                                break; // close on no more receivers
                            }
                        }
                    },
                    _ => {},
                }
            }
        }

        btleplug::Result::<()>::Ok(())
    };
            // }).await;
            // .then(|p| async move {
            //     match p {
            //         CentralEvent::DeviceDisconnected(_) => {},
            //         Centr
            //     }
                

            //     let props = p.properties().await?.unwrap();
            //     eprintln!("\t{} - {:?}", p.address(), props.local_name);
            //     // if p.services().is_empty() {
            //     //     let is_aranet = props.local_name.as_ref().map(|s| s.starts_with("Aranet4 ")).unwrap_or(false);

            //     //     match p.discover_services().await {
            //     //         Err(btleplug::Error::NotConnected) if is_aranet => {
            //     //             eprintln!("Found {}, but the device isn't paired. Please pair in your devices settings.",
            //     //                 props.local_name.as_ref().map::<&dyn Display, _>(|s| &*s).unwrap_or_else(|| &props.address)
            //     //             );
            //     //         },
            //     //         Ok(()) | Err(btleplug::Error::NotConnected) => {},
            //     //         Err(e) => return Err(e), // unknown error
            //     //     }
            //     // }
            //     Ok((p, props))
            // })
            // .filter(|(p, props)|)

        // instead of waiting, you can use adapter.events() to get a stream which will
        // notify you of new devices, for an example of that see btleplug/examples/event_driven_discovery.rs
        // time::sleep(Duration::from_secs(2)).await;

        // find the device we're interested in
        // let peripherals: Vec<Peripheral> = adapter.peripherals().await.unwrap();
        // eprintln!("found {} devices:", peripherals.len());
        // for p in peripherals {
        //     let props = p.properties().await?.unwrap();
        //     eprintln!("\t{} - {:?}", p.address(), props.local_name);
        //     let is_aranet = props.local_name.as_ref().map(|s| s.starts_with("Aranet4 ")).unwrap_or(false);
        //     if p.services().is_empty() {
        //         match p.discover_services().await {
        //             Err(btleplug::Error::NotConnected) if is_aranet => {
        //                 eprintln!("Found {}, but the device isn't paired. Please pair in your devices settings.",
        //                     props.local_name.as_ref().map::<&dyn Display, _>(|s| &*s).unwrap_or_else(|| &props.address)
        //                 );
        //             },
        //             Ok(()) | Err(btleplug::Error::NotConnected) => {},
        //             Err(e) => return Err(e), // unknown error
        //         }
        //     }

        //     // if ! p.services().iter().any(|s| s.uuid == aranet::uuids::AR4_SERVICE) { continue; }
            
            
        //     // // we prefiltered on the aranet4 service, so it has the Aranet4 service, so it is an Aranet4
        //     if props.local_name.map(|s| s.starts_with("Aranet4 ")).unwrap_or(false) {
        //         return Ok(Some(p));
        //     }
        // }
    // }

    let (res_disc, res_list) = tokio::join!(data_discovery, data_listener);
    eprintln!("result_discovery: {:?}", res_disc);
    eprintln!("result_list: {:?}", res_list);

    Ok(None)
}

async fn find_aranet4_retry(manager: &Manager) -> btleplug::Result<Option<Peripheral>> {
    if let Ok(Some(periph)) = find_aranet4(manager).await {
        return Ok(Some(periph));
    }
    if let Ok(Some(periph)) = find_aranet4(manager).await {
        return Ok(Some(periph));
    }
    find_aranet4(manager).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let manager = Manager::new().await.unwrap();

    // try three times
    let periph = find_aranet4_retry(&manager).await.unwrap().unwrap();

    // connect to the device
    periph.connect().await?;

    println!("connected");

    let ar4 = aranet::Aranet4::new(periph.clone()).await?;

    println!("name: {:?}", ar4.name().await?);
    println!("sw version: {:?}", ar4.version().await?);
    println!("last update: {}s ago", ar4.last_update_age().await?);
    println!("current readings: {:?}", ar4.current_readings().await?);

    let curr = ar4.current_readings_details().await?;
    println!("{} - FW {} (Battery: {}%)", ar4.name().await?, ar4.version().await?, (curr.battery * 100.0) as u8);
    println!("Last update: {}s ago with {}s interval", curr.age, curr.interval);
    println!("CO2: {}ppm ({:?})", curr.co2_ppm.unwrap(), curr.status);
    println!("Temp: {}°C ({}°F)", curr.temperature_c.unwrap(), curr.temperature_f().unwrap());
    println!("Humidity: {}%", (curr.humidity * 100.0) as u8);
    let pressure_atm = curr.pressure_hpa.unwrap() / 1013.25;
    println!("Pressure: {:.1} hPa ({:.3} atm)", curr.pressure_hpa.unwrap(), pressure_atm);

    println!("done");


    Ok(())
}