// No finding: requests go through network/, and a log target naming reqwest
// is a string.
use crate::network::reports;

fn setup(dispatch: fern::Dispatch) -> fern::Dispatch {
    dispatch
        .level_for("reqwest::connect", log::LevelFilter::Warn)
        .level_for("reqwest::retry", log::LevelFilter::Warn)
}

async fn send() {
    // A comment that mentions reqwest (the crate) is fine.
    let _ = reports::upload_report;
}
