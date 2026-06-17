use crate::versioned::event3::{EventV3, PreEventV3, RumorV3};
use crate::versioned::zap_data::ZapDataV2;

/// The main event type
pub type Event = EventV3;

/// Data used to construct an event
pub type PreEvent = PreEventV3;

/// A Rumor is an Event without a signature
pub type Rumor = RumorV3;

/// Data about a Zap
pub type ZapData = ZapDataV2;
