// TODO: TEST, THEN ADD POSTGRES SUPPORT
use crate::{info, interfaces::SimArbResultBatch, Result};
use std::{
    fs::File,
    io::{BufWriter, Write},
};

// for saving files
const EXPORT_DIR: &'static str = "./arbData";

fn parse_filename(filename: Option<String>) -> Result<String> {
    let filename = filename.unwrap_or(format!(
        "arbs_{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    ));
    Ok(if filename.ends_with(".json") {
        filename.to_owned()
    } else {
        format!("{}.json", filename)
    })
}

pub async fn save_arbs_to_file(
    filename: Option<String>,
    arbs: Vec<SimArbResultBatch>,
) -> Result<()> {
    let filename = parse_filename(filename)?;
    // create ./arbData/ if it doesn't exist
    tokio::fs::create_dir_all(EXPORT_DIR).await?;
    let filename = format!("{}/{}", EXPORT_DIR, filename);
    if arbs.len() > 0 {
        info!("exporting {} arbs to file {}...", arbs.len(), filename);
        let file = File::options()
            .append(true)
            .create(true)
            .open(filename.to_owned())?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &arbs)?;
        writer.flush()?;
    } else {
        info!("no arbs found to export.");
    }
    Ok(())
}
