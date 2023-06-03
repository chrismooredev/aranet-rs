
use std::error::Error;
use std::time::Duration;

use futures::StreamExt;
use btleplug::platform::Manager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    // Create our bluetooth adapter manager
    let manager = Manager::new().await.unwrap();

    // Create a stream of discovered Aranet4 devices
    let mut discovered = aranet::discover_aranet4(&manager).await?;

    // Find the next bluetooth advertisement for this device.
    // The .await may never resolve if there are no Aranet4 devices in the area.
    let advertisement = discovered.next().await.expect("unable to find Aranet4");

    if let Some(cr) = advertisement.current_reading {
        // The Aranet4 sends a current reading in it's advertisements.
        println!("Advertised reading:\n{}", cr);

        // The default Aranet4 sample interval is 300 seconds, or 5 minutes.
        // If we got data in the initial advertisement, wait for the next sample.
        tokio::time::sleep(Duration::from_secs(310)).await;
    }

    let aranet = advertisement.upgrade().await.expect("Unable to create Aranet4 device from advertisement");

    // this one breaks and i'm not sure whos fault it is
    // maybe try with the python lib or attempt to spy on the mobile app?
    // let cr = aranet.current_readings().await.expect("unable to read current details");

    let cr = aranet.current_readings_details().await.expect("unable to read current reading with details");

    println!("Fetched reading:\n{:?}", cr);

    Ok(())
}
