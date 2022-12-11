use serde::Serialize;
use crate::{BusMessage, GLOBALS, KeyPasswordPacket, PasswordPacket, Settings};
use crate::db::{DbPerson, DbPersonRelay, DbRelay};
use nostr_proto::{PrivateKey, PublicKeyHex};

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
        log::error!("Unable to send settings changed command: {}", e);
    }

    Ok(true)
}

#[tauri::command]
pub async fn follow_nip35(_address: String) -> Result<bool, String> {
    Err("Not Yet Implemented".to_string())
}

#[tauri::command]
pub async fn follow_key_and_relay(pubkey: String, relay: String) -> Result<DbPerson, String> {

    let pubkeyhex = PublicKeyHex(pubkey.clone());

    // Create or update them
    let person = match DbPerson::fetch_one(pubkeyhex.clone())
        .await
        .map_err(|e| format!("{}", e))?
    {
        Some(mut person) => {
            person.followed = 1;
            DbPerson::update(person.clone())
                .await
                .map_err(|e| format!("{}", e))?;
            person
        }
        None => {
            let person = DbPerson {
                pubkey: pubkeyhex.clone(),
                name: None,
                about: None,
                picture: None,
                dns_id: None,
                dns_id_valid: 0,
                dns_id_last_checked: None,
                followed: 1
            };
            DbPerson::insert(person.clone())
                .await
                .map_err(|e| format!("{}", e))?;
            person
        }
    };

    // Insert (or ignore) this relay
    DbRelay::insert(DbRelay {
        url: relay.clone(),
        success_count: 0,
        failure_count: 0,
        rank: Some(3)
    }).await.map_err(|e| format!("{}", e))?;

    // Insert (or ignore) this person's relay
    DbPersonRelay::insert(DbPersonRelay {
        person: pubkey,
        relay: relay,
        recommended: 0,
        last_fetched: None
    }).await.map_err(|e| format!("{}", e))?;

    // Tell the overlord to update the  minion to watch for their events
    // possibly starting a new minion if necessary.
    // FIXME TODO

    // Reply to javascript with the person which will be set in the store
    Ok(person)
}

#[tauri::command]
pub async fn follow_author() -> Result<DbPerson, String> {
    let public_key = "ee11a5dff40c19a555f41fe42b48f00e618c91225622ae37b6c2bb67b76c4e49".to_string();
    let relay = "wss://nostr.mikedilger.com".to_string();
    let person = follow_key_and_relay(public_key.clone(), relay.clone()).await?;

    for &relay in [
        "wss://nostr-pub.wellorder.net",
        "wss://nostr-relay.wlvs.space",
        "wss://nostr.onsats.org",
	    "wss://nostr.oxtr.dev",
	    "wss://relay.nostr.info"
    ].iter() {
        DbRelay::insert(DbRelay {
            url: relay.to_string(),
            success_count: 0,
            failure_count: 0,
            rank: Some(3)
        }).await.map_err(|e| format!("{}", e))?;

        DbPersonRelay::insert(DbPersonRelay {
            person: public_key.clone(),
            relay: relay.to_string(),
            recommended: 0,
            last_fetched: None
        }).await.map_err(|e| format!("{}", e))?;
    }

    // Tell the overlord to update the  minion to watch for their events
    // possibly starting a new minion if necessary.
    // FIXME TODO

    Ok(person)
}

#[tauri::command]
pub async fn generate(password: String) -> Result<(), String> {
    let password_packet = PasswordPacket(password);

    // Send it to the overlord
    let tx = GLOBALS.to_overlord.clone();
    if let Err(e) = tx.send(BusMessage {
        relay_url: None,
        target: "overlord".to_string(),
        kind: "generate".to_string(),
        payload: serde_json::to_string(&password_packet)
            .map_err(|e| format!("{}", e))?
    }) {
        log::error!("Unable to send password to the overlord for generate: {}", e);
    }

    Ok(())
}

#[tauri::command]
pub async fn unlock(password: String) -> Result<(), String> {
    let password_packet = PasswordPacket(password);

    // Send it to the overlord
    let tx = GLOBALS.to_overlord.clone();
    if let Err(e) = tx.send(BusMessage {
        relay_url: None,
        target: "overlord".to_string(),
        kind: "unlock".to_string(),
        payload: serde_json::to_string(&password_packet)
            .map_err(|e| format!("{}", e))?
    }) {
        log::error!("Unable to send password to the overlord for unlock: {}", e);
    }

    Ok(())
}

#[tauri::command]
// send back public key on success
pub async fn import_key(privatekey: String, password: String) -> Result<(), String> {
    // Verify the key is valid
    let _private_key_obj = PrivateKey::try_from_hex_string(&privatekey)
        .map_err(|e| format!("{}", e))?;

    let key_password_packet = KeyPasswordPacket(privatekey, password);

    // Send it to the overlord
    let tx = GLOBALS.to_overlord.clone();
    if let Err(e) = tx.send(BusMessage {
        relay_url: None,
        target: "overlord".to_string(),
        kind: "import_key".to_string(),
        payload: serde_json::to_string(&key_password_packet)
            .map_err(|e| format!("{}", e))?
    }) {
        log::error!("Unable to send imported key to the overlord: {}", e);
    }

    Ok(())
}
