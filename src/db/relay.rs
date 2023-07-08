use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Id, RelayInformationDocument, RelayUrl};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbRelay {
    pub url: RelayUrl,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_connected_at: Option<u64>,
    pub last_general_eose_at: Option<u64>,
    pub rank: u64,
    pub hidden: bool,
    pub usage_bits: u64,
    pub nip11: Option<RelayInformationDocument>,
    pub last_attempt_nip11: Option<u64>,
}

impl DbRelay {
    pub const READ: u64 = 1 << 0; // 1
    pub const WRITE: u64 = 1 << 1; // 2
    pub const ADVERTISE: u64 = 1 << 2; // 4
    pub const INBOX: u64 = 1 << 3; // 8
    pub const OUTBOX: u64 = 1 << 4; // 16
    pub const DISCOVER: u64 = 1 << 5; // 32

    const SQL_OUTBOX_IS_ON: &'static str = "(relay.usage_bits & 16 = 16)";

    pub fn new(url: RelayUrl) -> DbRelay {
        DbRelay {
            url,
            success_count: 0,
            failure_count: 0,
            last_connected_at: None,
            last_general_eose_at: None,
            rank: 3,
            hidden: false,
            usage_bits: 0,
            nip11: None,
            last_attempt_nip11: None,
        }
    }

    pub fn set_usage_bits_memory_only(&mut self, bits: u64) {
        self.usage_bits |= bits;
    }

    pub fn clear_usage_bits_memory_only(&mut self, bits: u64) {
        self.usage_bits &= !bits;
    }

    pub fn adjust_usage_bit_memory_only(&mut self, bit: u64, value: bool) {
        if value {
            self.set_usage_bits_memory_only(bit);
        } else {
            self.clear_usage_bits_memory_only(bit);
        }
    }

    pub fn has_usage_bits(&self, bits: u64) -> bool {
        self.usage_bits & bits == bits
    }

    pub async fn save_usage_bits(&self) -> Result<(), Error> {
        let sql = "UPDATE relay SET usage_bits = ? WHERE url = ?";
        let bits = self.usage_bits;
        let url = self.url.0.clone();
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((bits, url,)));
            Ok::<(), Error>(())
        })
        .await??;
        Ok(())
    }

    pub fn attempts(&self) -> u64 {
        self.success_count + self.failure_count
    }

    pub fn success_rate(&self) -> f32 {
        let attempts = self.success_count + self.failure_count;
        if attempts == 0 {
            return 0.5;
        } // unknown, so we put it in the middle
        self.success_count as f32 / attempts as f32
    }

    pub async fn fetch(criteria: Option<&str>) -> Result<Vec<DbRelay>, Error> {
        let sql = "SELECT url, success_count, failure_count, last_connected_at, \
                   last_general_eose_at, rank, hidden, usage_bits, \
                   nip11, last_attempt_nip11 FROM relay"
            .to_owned();
        let sql = match criteria {
            None => sql,
            Some(crit) => format!("{} WHERE {}", sql, crit),
        };

        let output: Result<Vec<DbRelay>, Error> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();

            let mut stmt = db.prepare(&sql)?;
            let mut rows = rtry!(stmt.query([]));
            let mut output: Vec<DbRelay> = Vec::new();
            while let Some(row) = rows.next()? {
                let s: String = row.get(0)?;

                // just skip over invalid relay URLs
                if let Ok(url) = RelayUrl::try_from_str(&s) {
                    let nip11: Option<String> = row.get(8)?;

                    output.push(DbRelay {
                        url,
                        success_count: row.get(1)?,
                        failure_count: row.get(2)?,
                        last_connected_at: row.get(3)?,
                        last_general_eose_at: row.get(4)?,
                        rank: row.get(5)?,
                        hidden: row.get(6)?,
                        usage_bits: row.get(7)?,
                        nip11: match nip11 {
                            None => None,
                            Some(s) => serde_json::from_str(&s)?,
                        },
                        last_attempt_nip11: row.get(9)?,
                    });
                }
            }
            Ok::<Vec<DbRelay>, Error>(output)
        })
        .await?;

        output
    }

    pub async fn fetch_one(url: &RelayUrl) -> Result<Option<DbRelay>, Error> {
        let relays = DbRelay::fetch(Some(&format!("url='{}'", url.0))).await?;

        if relays.is_empty() {
            Ok(None)
        } else {
            Ok(Some(relays[0].clone()))
        }
    }

    pub async fn insert(relay: DbRelay) -> Result<(), Error> {
        let sql = "INSERT OR IGNORE INTO relay (url, success_count, failure_count, \
                   last_connected_at, last_general_eose_at, rank, hidden, usage_bits, \
                   nip11, last_attempt_nip11) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";

        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();

            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((
                &relay.url.0,
                &relay.success_count,
                &relay.failure_count,
                &relay.last_connected_at,
                &relay.last_general_eose_at,
                &relay.rank,
                &relay.hidden,
                &relay.usage_bits,
                match relay.nip11 {
                    None => None,
                    Some(n11) => Some(serde_json::to_string(&n11)?),
                },
                &relay.last_attempt_nip11,
            )));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn update(relay: DbRelay) -> Result<(), Error> {
        let sql = "UPDATE relay SET success_count=?, failure_count=?, \
                   last_connected_at=?, last_general_eose_at=?, \
                   rank=?, hidden=?, usage_bits=?, nip11=?, last_attempt_nip11=? \
                   WHERE url=?";

        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();

            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((
                &relay.success_count,
                &relay.failure_count,
                &relay.last_connected_at,
                &relay.last_general_eose_at,
                &relay.rank,
                &relay.hidden,
                &relay.usage_bits,
                match relay.nip11 {
                    None => None,
                    Some(n11) => Some(serde_json::to_string(&n11)?),
                },
                &relay.last_attempt_nip11,
                &relay.url.0,
            )));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn clear_all_relay_list_usage_bits() -> Result<(), Error> {
        // Keep only bits which are NOT part of relay lists
        let sql = format!(
            "UPDATE relay SET usage_bits = usage_bits & {}",
            !(Self::INBOX | Self::OUTBOX)
        );

        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(&sql)?;
            rtry!(stmt.execute(()));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /// This also bumps success_count
    pub async fn update_general_eose(
        url: RelayUrl,
        last_general_eose_at: u64,
    ) -> Result<(), Error> {
        let sql =
            "UPDATE relay SET last_general_eose_at = max(?, ifnull(last_general_eose_at,0)) WHERE url = ?";

        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((&last_general_eose_at, &url.0)));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn update_hidden(url: RelayUrl, hidden: bool) -> Result<(), Error> {
        let sql = "UPDATE relay SET hidden = ?  WHERE url = ?";
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            rtry!(stmt.execute((&hidden, &url.0)));
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    /*
    pub async fn delete(criteria: &str) -> Result<(), Error> {
        let sql = format!("DELETE FROM relay WHERE {}", criteria);

        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            db.execute(&sql, [])?;
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }
     */

    pub async fn populate_new_relays() -> Result<(), Error> {
        // Get relays from person_relay list
        let sql =
            "INSERT OR IGNORE INTO relay (url, rank) SELECT DISTINCT relay, 3 FROM person_relay";

        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            db.execute(sql, [])?;
            Ok::<(), Error>(())
        })
        .await??;

        // Select relays from 'e' and 'p' event tags
        let sql = "SELECT DISTINCT field1 FROM event_tag where (label='e' OR label='p')";
        let urls: Vec<RelayUrl> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            let mut rows = rtry!(stmt.query([]));
            let mut maybe_urls: Vec<RelayUrl> = Vec::new();
            while let Some(row) = rows.next()? {
                let maybe_string: Option<String> = row.get(0)?;
                if let Some(string) = maybe_string {
                    if let Ok(url) = RelayUrl::try_from_str(&string) {
                        maybe_urls.push(url);
                    }
                }
            }
            Ok::<Vec<RelayUrl>, Error>(maybe_urls)
        })
        .await??;

        // FIXME this is a lot of separate sql calls
        spawn_blocking(move || {
            let sql = "INSERT OR IGNORE INTO RELAY (url, rank) VALUES (?, 3)";
            for url in urls {
                let db = GLOBALS.db.blocking_lock();
                db.execute(sql, [&url.0])?;
            }
            Ok::<(), Error>(())
        })
        .await??;

        Ok(())
    }

    pub async fn recommended_relay_for_reply(reply_to: Id) -> Result<Option<RelayUrl>, Error> {
        // FIXME - USE THE INBOX FOR THE USER, NOT THE SEEN ON RELAY

        // Try to find a relay where the event was seen AND that is an outbox to which
        // has a rank>1
        let sql = format!(
            "SELECT url FROM relay INNER JOIN event_relay ON relay.url=event_relay.relay \
             WHERE event_relay.event=? AND {} AND relay.rank>1",
            Self::SQL_OUTBOX_IS_ON
        );
        let output: Option<RelayUrl> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(&sql)?;
            let mut query_result = rtry!(stmt.query([reply_to.as_hex_string()]));
            if let Some(row) = query_result.next()? {
                let s: String = row.get(0)?;
                let url = RelayUrl::try_from_str(&s)?;
                Ok::<Option<RelayUrl>, Error>(Some(url))
            } else {
                Ok::<Option<RelayUrl>, Error>(None)
            }
        })
        .await??;

        if output.is_some() {
            return Ok(output);
        }

        // Fallback to finding any relay where the event was seen
        let sql = "SELECT relay FROM event_relay WHERE event=?";
        let output: Option<RelayUrl> = spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(sql)?;
            let mut query_result = rtry!(stmt.query([reply_to.as_hex_string()]));
            if let Some(row) = query_result.next()? {
                let s: String = row.get(0)?;
                let url = RelayUrl::try_from_str(&s)?;
                Ok::<Option<RelayUrl>, Error>(Some(url))
            } else {
                Ok::<Option<RelayUrl>, Error>(None)
            }
        })
        .await??;

        Ok(output)
    }

    /*
    pub async fn delete_relay(url: RelayUrl) -> Result<(), Error> {
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let _ = db.execute("DELETE FROM event seen WHERE relay=?", (&url.0,));
            let _ = db.execute("UPDATE contact SET relay=null WHERE relay=?", (&url.0,));
            let _ = db.execute("DELETE FROM person_relay WHERE relay=?", (&url.0,));
            let _ = db.execute("DELETE FROM relay WHERE url=?", (&url.0,));
        })
            .await?;

        Ok(())
    }
     */

    pub async fn set_rank(url: RelayUrl, rank: u8) -> Result<(), Error> {
        spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
            let _ = db.execute("UPDATE relay SET rank=? WHERE url=?", (&rank, &url.0));
        })
        .await?;

        Ok(())
    }
}
