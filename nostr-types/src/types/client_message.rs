use crate::types::{Event, Filter, SubscriptionId};
use serde::de::Error as DeError;
use serde::de::{Deserialize, Deserializer, IgnoredAny, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};
use std::fmt;

/// A message from a client to a relay
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientMessage {
    /// An event
    Event(Box<Event>),

    /// A subscription request
    Req(SubscriptionId, Filter),

    /// A request to close a subscription
    Close(SubscriptionId),

    /// Used to send authentication events
    Auth(Box<Event>),

    /// Count
    Count(SubscriptionId, Filter),

    /// Negentropy Initiation
    NegOpen(SubscriptionId, Filter, String),

    /// Negentropy Message
    NegMsg(SubscriptionId, String),

    /// Negentropy Close
    NegClose(SubscriptionId),
}

impl ClientMessage {
    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) async fn mock() -> ClientMessage {
        ClientMessage::Event(Box::new(Event::mock().await))
    }
}

impl Serialize for ClientMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ClientMessage::Event(event) => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element("EVENT")?;
                seq.serialize_element(&event)?;
                seq.end()
            }
            ClientMessage::Req(id, filter) => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element("REQ")?;
                seq.serialize_element(&id)?;
                seq.serialize_element(&filter)?;
                seq.end()
            }
            ClientMessage::Close(id) => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element("CLOSE")?;
                seq.serialize_element(&id)?;
                seq.end()
            }
            ClientMessage::Auth(event) => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element("AUTH")?;
                seq.serialize_element(&event)?;
                seq.end()
            }
            ClientMessage::Count(id, filter) => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element("COUNT")?;
                seq.serialize_element(&id)?;
                seq.serialize_element(&filter)?;
                seq.end()
            }
            ClientMessage::NegOpen(subid, filter, msg) => {
                let mut seq = serializer.serialize_seq(Some(4))?;
                seq.serialize_element("NEG-OPEN")?;
                seq.serialize_element(&subid)?;
                seq.serialize_element(&filter)?;
                seq.serialize_element(&msg)?;
                seq.end()
            }
            ClientMessage::NegMsg(subid, msg) => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element("NEG-MSG")?;
                seq.serialize_element(&subid)?;
                seq.serialize_element(&msg)?;
                seq.end()
            }
            ClientMessage::NegClose(subid) => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element("NEG-CLOSE")?;
                seq.serialize_element(&subid)?;
                seq.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ClientMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(ClientMessageVisitor)
    }
}

struct ClientMessageVisitor;

impl<'de> Visitor<'de> for ClientMessageVisitor {
    type Value = ClientMessage;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a sequence of strings")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<ClientMessage, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let word: &str = seq
            .next_element()?
            .ok_or_else(|| DeError::custom("Message missing initial string field"))?;
        let mut output: Option<ClientMessage> = None;
        if word == "EVENT" {
            let event: Event = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing event field"))?;
            output = Some(ClientMessage::Event(Box::new(event)))
        } else if word == "REQ" {
            let id: SubscriptionId = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing id field"))?;
            let filter: Filter = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing filter field"))?;
            output = Some(ClientMessage::Req(id, filter))
        } else if word == "COUNT" {
            let id: SubscriptionId = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing filter field"))?;
            let filter: Filter = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing filter field"))?;
            output = Some(ClientMessage::Count(id, filter))
        } else if word == "CLOSE" {
            let id: SubscriptionId = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing id field"))?;
            output = Some(ClientMessage::Close(id))
        } else if word == "AUTH" {
            let event: Event = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing event field"))?;
            output = Some(ClientMessage::Auth(Box::new(event)))
        } else if word == "NEG-OPEN" {
            let id: SubscriptionId = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing id field"))?;
            let filter: Filter = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing filter"))?;
            let msg: String = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing message"))?;
            output = Some(ClientMessage::NegOpen(id, filter, msg))
        } else if word == "NEG-MSG" {
            let id: SubscriptionId = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing id field"))?;
            let msg: String = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing message"))?;
            output = Some(ClientMessage::NegMsg(id, msg))
        } else if word == "NEG-CLOSE" {
            let id: SubscriptionId = seq
                .next_element()?
                .ok_or_else(|| DeError::custom("Message missing id field"))?;
            output = Some(ClientMessage::NegClose(id))
        }

        // Consume any trailing fields
        while let Some(_ignored) = seq.next_element::<IgnoredAny>()? {}

        match output {
            Some(cm) => Ok(cm),
            None => Err(DeError::custom(format!("Unknown Message: {word}"))),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Event;

    test_serde_async! {ClientMessage, test_client_message_serde}

    test_serde_val_async! {
        test_client_message_serde_event,
        ClientMessage::Event(Box::new(Event::mock().await))
    }
    test_serde_val! {
        test_client_message_serde_req,
        ClientMessage::Req(SubscriptionId::mock(), Filter::mock())
    }
    test_serde_val! {
        test_client_message_serde_close,
        ClientMessage::Close(SubscriptionId::mock())
    }
    test_serde_val_async! {
        test_client_message_serde_auth,
        ClientMessage::Auth(Box::new(Event::mock().await))
    }
    test_serde_val! {
        test_client_message_serde_negopen,
        ClientMessage::NegOpen(SubscriptionId::mock(), Filter::mock(), "dummy".to_string())
    }
    test_serde_val! {
        test_client_message_serde_negmsg,
        ClientMessage::NegMsg(SubscriptionId::mock(), "dummy".to_string())
    }
    test_serde_val! {
        test_client_message_serde_negclose,
        ClientMessage::NegClose(SubscriptionId::mock())
    }
}
