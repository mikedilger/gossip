use serde::Serialize;
use crate::{BusMessage, GLOBALS, Settings};
use crate::db::{DbPerson, DbPersonRelay, DbRelay};
use nostr_proto::PublicKeyHex;

#[derive(Debug, Serialize)]
pub struct About {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: String,
    pub repository: String,
    pub homepage: String,
    pub license: String,
    pub database_path: String,
}

#[tauri::command]
pub fn about() -> About {
    let data_dir = match dirs::data_dir() {
        Some(mut d) => {
            d.push("gossip");
            d.push("gossip.sqlite");
            format!("{}", d.display())
        },
        None =>
            "Cannot find a directory to store application data.".to_owned()
    };

    About {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: env!("CARGO_PKG_DESCRIPTION").to_string(),
        authors: env!("CARGO_PKG_AUTHORS").to_string(),
        repository: env!("CARGO_PKG_REPOSITORY").to_string(),
        homepage: env!("CARGO_PKG_HOMEPAGE").to_string(),
        license: env!("CARGO_PKG_LICENSE").to_string(),
        database_path: data_dir.to_string(),
    }
}

#[tauri::command]
pub fn javascript_is_ready() {
    let tx = GLOBALS.to_overlord.clone();

    log::debug!("javascript-is-ready tauri command called");

    if let Err(e) = tx.send(BusMessage {
        relay_url: None,
        target: "overlord".to_string(),
        kind: "javascript_is_ready".to_string(),
        payload: "".to_string()
    }) {
        log::error!("Unable to send javascript_is_ready: {}", e);
    }
}

#[tauri::command]
pub async fn save_settings(settings: Settings) -> Result<bool, String> {
    settings.save().await.map_err(|e| format!("{}", e))?;

    let tx = GLOBALS.to_overlord.clone();
    if let Err(e) = tx.send(BusMessage {
        relay_url: None,
        target: "overlord".to_string(),
        kind: "settings_changed".to_string(),
        payload: serde_json::to_string(&settings).map_err(|e| format!("{}", e))?
    }) {
        log::error!("Unable to send javascript_is_ready: {}", e);
    }


    Ok(true)
}

#[tauri::command]
pub async fn follow_nip35(_address: String) -> Result<bool, String> {
    Err("Not Yet Implemented".to_string())
}

#[tauri::command]
pub async fn follow_key_and_relay(pubkey: String, relay: String) -> Result<bool, String> {
    DbPerson::insert(DbPerson {
        pubkey: PublicKeyHex(pubkey.clone()),
        name: None,
        about: None,
        picture: None,
        dns_id: None,
        dns_id_valid: 0,
        dns_id_last_checked: None,
        followed: 1
    }).await.map_err(|e| format!("{}", e))?;

    DbPersonRelay::insert(DbPersonRelay {
        person: pubkey,
        relay: relay.clone(),
        recommended: 0,
        last_fetched: None
    }).await.map_err(|e| format!("{}", e))?;

    DbRelay::insert(DbRelay {
        url: relay.clone(),
        success_count: 0,
        failure_count: 0,
        rank: Some(3)
    }).await.map_err(|e| format!("{}", e))?;

    Ok(true)
}

#[tauri::command]
pub async fn follow_author() -> Result<bool, String> {

    let pk = "ee11a5dff40c19a555f41fe42b48f00e618c91225622ae37b6c2bb67b76c4e49";

    DbPerson::insert(DbPerson {
        pubkey: PublicKeyHex(pk.to_owned()),
        name: None,
        about: None,
        picture: None,
        dns_id: None,
        dns_id_valid: 0,
        dns_id_last_checked: None,
        followed: 1
    }).await.map_err(|e| format!("{}", e))?;

    for &relay in [
        "wss://nostr-pub.wellorder.net",
        "wss://nostr-relay.wlvs.space",
        //"wss://nostr.onsats.org",
    ].iter() {
        DbPersonRelay::insert(DbPersonRelay {
            person: pk.to_owned(),
            relay: relay.to_owned(),
            recommended: 0,
            last_fetched: None
        }).await.map_err(|e| format!("{}", e))?;

        DbRelay::insert(DbRelay {
            url: relay.to_owned(),
            success_count: 0,
            failure_count: 0,
            rank: Some(3)
        }).await.map_err(|e| format!("{}", e))?;
    }

    Ok(true)
}
