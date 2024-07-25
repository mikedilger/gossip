# Gossip Commands

Gossip has a lot of command-line commands that can be run.  To run a command line command,
pass it to gossip on the command line, for example:

```
$ gossip wgpu_renderer true
```

Most command line commands do something and then exit, rather than running the GUI program.

This file may not be up to date. You can see the list of all the commands with `gossip help`.

You can see the details of a particular command with `gossip help <command>`.

## Commands that launce the GUI program

### login

login on the command line before starting the gossip GUI.

**usage**:   `gossip login`

### rapid

Use much faster disk access. A crash can corrupt your local data, unless your filesystem preserves write ordering.

**usage**:   `gossip rapid`

Unlike all the other commands, you can pass one more command after rapid like this:

**usage**:   `gossip rapid <command>`

### offline

Start gossip in offline mode.

**usage**:   `gossip offline`

## Commands that operate in the terminal and then exit

### add_person_list

add a new person list with the given name.

**usage**:  `gossip add_person_list <listname>`

### backdate_eose

backdate last_general_eose_at by 24 hours for every relay.

**usage**:  `gossip backdate_eose`

### bech32_decode

decode the bech32 string.

**usage**:  `gossip bech32_decode <bech32string>`

### bech32_encode_naddr

encode an event address (parameterized replaceable event link).

**usage**:  `gossip bech32_encode_naddr <kind> <pubkeyhex> <d> [<relayurl>, ...]`

### clear_timeouts

clear relay avoidance timeouts.

**usage**:  `gossip clear_timeouts`

### decrypt

decrypt the ciphertext from the pubkeyhex.

**usage**:  `gossip decrypt <pubkeyhex> <ciphertext>`

### delete_spam_by_content

delete all feed-displayable events with content matching the substring (including giftwraps).

**usage**:  `gossip delete_spam_by_content`

### delete_relay

delete a relay record from storage.

**usage**:  `gossip delete_relay <relayurl>`

### dpi

override the DPI setting.

**usage**:  `gossip dpi <dpi>`

### events_of_kind

print IDs of all events of kind=<kind>

**usage**:  `gossip events_of_kind <kind>`

### events_of_pubkey_and_kind

print IDs of all events from <pubkeyhex> of kind=<kind>

**usage**:  `gossip events_of_pubkey_and_kind <pubkeyhex> <kind>`

### export_encrypted_key

Export the encrypted private key

**usage**:  `gossip export_encrypted_key`

### giftwraps

Decrypt all of your giftwraps

**usage**:  `gossip giftwraps`

### help

Show this list.

**usage**:  `gossip help`

### import_encrypted_private_key

import encrypted private key

**usage**:  `gossip import_encrypted_private_key <ncryptsec>`

### import_event

import and process a JSON event

**usage**:  `gossip import_event <event_json>`

### print_event

print the event (in JSON) from the database that has the given id

**usage**:  `gossip print_event <idhex>`

### print_followed

print every pubkey that is followed

**usage**:  `gossip print_followed`

### print_muted

print every pubkey that is muted

**usage**:  `gossip print_muted`

### print_person_lists

print every pubkey in every person list

**usage**:  `gossip print_person_lists`

### print_person

print the given person

**usage**:  `gossip print_person <pubkeyHexOrBech32>`

### print_person_relays

print all the person-relay records for the given person

**usage**:  `gossip print_person_relays <pubkeyhex>`

### print_relay

print the relay record

**usage**:  `gossip print_relay <url>`

### print_relays

print all the relay records

**usage**:  `gossip print_relays`


### print_seen_on

print the relays the event was seen on

**usage**:  `gossip print_seen_on <idhex>`

### rebuild_indices

Rebuild all event-related indices

**usage**:  `gossip rebuild_indices`

### rename_person_list

Rename a person list

**usage**:  `gossip rename_person_list`

### reprocess_recent

Reprocess events that came during the last 24 hours

**usage**:  `gossip reprocess_recent`

### reprocess_relay_lists

Reprocess relay lists (including kind 3 contents)

**usage**:  `gossip reprocess_relay_lists`

### ungiftwrap

Unwrap the giftwrap event with the given ID and print the rumor (in JSON)

**usage**:  `gossip ungiftwrap <idhex>`

### verify

Verify if the given event signature is valid

**usage**:  `gossip verify <idhex>`

Note that gossip does not accept invalid events, so this old command probably has no practical use.

### verify_json

Verify if the passed in event JSON's signature is valid

**usage**:  `gossip verify_json <event_json>`

### wgpu_renderer

Enable/Disable the WGPU rendering backend

**usage**:  `gossip wgpu_renderer <true | false>`
