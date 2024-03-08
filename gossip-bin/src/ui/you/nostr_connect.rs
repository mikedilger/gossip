use super::GossipUi;
use crate::ui::widgets::CopyButton;
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Nip46UnconnectedServer, GLOBALS};
use nostr_types::RelayUrl;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Nostr Connect");
    });

    ui.add_space(10.0);
    ui.label("NOTE: Gossip currently only acts as a signing service, using the key you have configured in gossip.");

    // Show status of unconnected server
    if let Ok(Some(unconnected_server)) = GLOBALS.storage.read_nip46_unconnected_server() {
        ui.separator();
        ui.add_space(10.0);
        ui.heading("Service is Waiting for Client to Connect");

        ui.add_space(10.0);
        // Status of unconnected server
        ui.label("A Service is waiting for a NostrConnect connection at the following relays:");
        for relay in &unconnected_server.relays {
            ui.label(format!("        {}", relay));
        }

        // Show token
        if let Ok(token) = unconnected_server.connection_token() {
            ui.label("Use this secret string at the client to connect:");
            ui.horizontal_wrapped(|ui| {
                ui.label("   ");
                ui.label(&token);
                if ui.add(CopyButton::new()).clicked() {
                    ui.output_mut(|o| o.copied_text = token);
                }
            });
        }

        // Allow delete
        if ui.button("Delete this unconnected service").clicked() {
            let _ = GLOBALS.storage.delete_nip46_unconnected_server(None);
        }
    } else {
        setup_unconnected_service(app, ui);
    }

    // Connected servers
    if let Ok(servers) = GLOBALS.storage.read_all_nip46servers() {
        if !servers.is_empty() {
            ui.separator();
            ui.add_space(10.0);
            ui.heading("Connected Services");
            ui.add_space(10.0);
        }
        for server in &servers {
            let peer = server.peer_pubkey.as_bech32_string();
            ui.label(format!("name={}, Peer={}", server.name, peer));
            if ui.button("Disconnect").clicked() {
                let _ = GLOBALS.storage.delete_nip46server(server.peer_pubkey, None);
            }
        }
    }

    ui.separator();
}

fn setup_unconnected_service(app: &mut GossipUi, ui: &mut Ui) {
    ui.separator();
    ui.add_space(10.0);
    ui.heading("Setup a Service");

    /*
        NIP-46 doesn't explain well enough how this can work. Servers can't send methods
        to clients, yet it wants us to send a connect method to the client. Until that is
        resolved, this method of setup is not available.

        ui.add_space(10.0);
        ui.label("OPTION 1: Paste a secret string from the client:");
        ui.add(text_edit_line!(app, app.nostr_connect_string));
        if !app.nostr_connect_string.is_empty() {
        if ui.button("CONNECT").clicked() {
        match Nip46Server::new_from_client(app.nostr_connect_string.clone()) {
        Ok(server) => {
        // GINA - save server
        // GINA - server needs to send 'connect' to the client
    },
        Err(e) => {
        GLOBALS.status_queue.write().write(format!("{}", e));
    }
    }
        app.nostr_connect_string = "".to_owned();
    }
    }
         */

    ui.add_space(10.0);
    ui.label("Enter a name for the client that will be connecting:");

    ui.horizontal(|ui| {
        ui.label("Name: ");
        ui.add(text_edit_line!(app, app.nostr_connect_name));
    });

    ui.add_space(10.0);
    ui.label("Enter 1 or 2 relays to do nostr-connect over:");

    ui.horizontal(|ui| {
        ui.label("Relay: ");
        ui.add(text_edit_line!(app, app.nostr_connect_relay1));
    });
    ui.horizontal(|ui| {
        ui.label("Relay: ");
        ui.add(text_edit_line!(app, app.nostr_connect_relay2));
    });

    if !app.nostr_connect_name.is_empty() && !app.nostr_connect_relay1.is_empty() {
        if let Ok(relay1) = RelayUrl::try_from_str(&app.nostr_connect_relay1) {
            if !app.nostr_connect_relay2.is_empty() {
                if let Ok(relay2) = RelayUrl::try_from_str(&app.nostr_connect_relay2) {
                    if ui.button("Create Service").clicked() {
                        create_service(app.nostr_connect_name.clone(), vec![relay1, relay2]);
                        app.nostr_connect_name = "".to_string();
                    }
                }
            } else if ui.button("Create Service").clicked() {
                create_service(app.nostr_connect_name.clone(), vec![relay1]);
                app.nostr_connect_name = "".to_string();
            }
        }
    }
}

fn create_service(name: String, relays: Vec<RelayUrl>) {
    // Create the unconnected server (1 relay)
    let server = Nip46UnconnectedServer::new(name, relays.clone());

    // Store it
    let _ = GLOBALS
        .storage
        .write_nip46_unconnected_server(&server, None);

    // Tell the overlord to subscribe
    let _ = GLOBALS
        .to_overlord
        .send(ToOverlordMessage::SubscribeNip46(relays));
}
