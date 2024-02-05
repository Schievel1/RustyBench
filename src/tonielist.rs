use std::{path::PathBuf, sync::Arc};

use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref TONIES: Arc<Result<Vec<Tonie>, Error>> = Arc::new(Ok(Vec::new()));
}

pub type ToniesRoot = Vec<Tonie>;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tonie {
    pub article: String,
    pub data: Vec<Daum>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Daum {
    pub series: Option<String>,
    pub episode: Option<String>,
    pub release: i64,
    pub language: Option<String>,
    pub category: Option<String>,
    pub runtime: i64,
    pub age: i64,
    pub origin: String,
    pub image: Option<String>,
    pub sample: Option<String>,
    pub web: Option<String>,
    #[serde(rename = "shop-id")]
    pub shop_id: Option<String>,
    #[serde(rename = "track-desc")]
    pub track_desc: Vec<String>,
    pub ids: Vec<Id>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Id {
    #[serde(rename = "audio-id")]
    pub audio_id: i64,
    pub hash: String,
    pub size: i64,
    pub tracks: i64,
    pub confidence: i64,
}

pub fn get_tonie_list_online(custom_url: Option<&str>) -> Result<ToniesRoot> {
    let mut url =
        "https://raw.githubusercontent.com/toniebox-reverse-engineering/tonies-json/release/toniesV2.json";
    if let Some(custom_url) = custom_url {
        url = custom_url;
    }
    // need to do this C style because it is evaluated at start using lazy_static
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build();
    if let Err(e) = client {
        log::error!("Failed to get tonie list: {}", e);
        return Err(e.into());
    }
    let response = client.unwrap().get(url).send();
    if let Err(e) = response {
        log::error!("Failed to get tonie list: {}", e);
        return Err(e.into());
    }
    let body = response.unwrap().text();
    if let Err(e) = body {
        log::error!("Failed to get tonie list: {}", e);
        return Err(e.into());
    }
    let tonies = serde_json::from_str::<Vec<Tonie>>(&body.unwrap());
    if let Err(e) = tonies {
        log::error!("Failed to get tonie list: {}", e);
        return Err(e.into());
    }
    Ok(tonies.unwrap())
}

pub fn get_tonie_list_from_file(path: PathBuf) -> Result<ToniesRoot> {
    let tonies = std::fs::read_to_string(path);
    if let Err(e) = tonies {
        log::error!("Failed to get tonie list: {}", e);
        return Err(e.into());
    }
    let tonies = serde_json::from_str::<Vec<Tonie>>(&tonies.unwrap());
    if let Err(e) = tonies {
        log::error!("Failed to get tonie list: {}", e);
        return Err(e.into());
    }
    Ok(tonies.unwrap())
}

pub fn find_tonie_with_audio_id(tonielist: &Arc<Vec<Tonie>>, audio_id: u32) -> Option<Tonie> {
    for tonie in tonielist.as_slice() {
        for daum in &tonie.data {
            for id in &daum.ids {
                if id.audio_id == audio_id as i64 {
                    return Some(tonie.clone());
                }
            }
        }
    }
    None
}
