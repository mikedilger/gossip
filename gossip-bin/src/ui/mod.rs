macro_rules! text_edit_line {
    ($app:ident, $var:expr) => {
        crate::ui::widgets::TextEdit::singleline(&$app.theme, &mut $var)
            .text_color($app.theme.input_text_color())
    };
}

macro_rules! text_edit_multiline {
    ($app:ident, $var:expr) => {
        egui::widgets::TextEdit::multiline(&mut $var).text_color($app.theme.input_text_color())
    };
}

macro_rules! btn_h_space {
    ($ui:ident) => {
        $ui.add_space(20.0)
    };
}

macro_rules! read_setting {
    ($field:ident) => {
        paste::paste! {
            gossip_lib::GLOBALS.db().[<read_setting_ $field>]()
        }
    };
}

macro_rules! write_setting {
    ($field:ident, $val:expr) => {
        paste::paste! {
            let _ = gossip_lib::GLOBALS.db().[<write_setting_ $field>](&$val, None);
        }
    };
}

mod assets;
mod dm_chat_list;
mod emojis;
mod feed;
mod handler;
mod help;
mod notifications;
mod people;
mod relays;
mod search;
mod settings;
mod theme;
mod widgets;
mod wizard;
mod you;

use crate::about::About;
pub use crate::ui::theme::{Theme, ThemeVariant};
use crate::unsaved_settings::UnsavedSettings;
#[cfg(feature = "video-ffmpeg")]
use core::cell::RefCell;
use eframe::egui;
use eframe::egui::vec2;
use eframe::egui::FontId;
use egui::widgets::Slider;
use egui::{
    Align, Color32, ColorImage, Context, IconData, Image, ImageData, Label, Layout, RichText,
    ScrollArea, Sense, TextureHandle, TextureOptions, Ui, Vec2,
};
use egui_file_dialog::FileDialog;
#[cfg(feature = "video-ffmpeg")]
use egui_video::{AudioDevice, Player};
use egui_winit::egui::Rect;
use egui_winit::egui::Response;
use egui_winit::egui::ViewportBuilder;
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{
    DmChannel, DmChannelData, Error, FeedKind, MediaLoadingResult, Person, PersonList, Private,
    RunState, ZapState, GLOBALS,
};
use handler::Handlers;
use nostr_types::ContentSegment;
use nostr_types::RelayUrl;
use nostr_types::{
    EventKind, FileMetadata, Id, Metadata, MilliSatoshi, Profile, PublicKey, UncheckedUrl, Url,
};
use widgets::ModalEntry;

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

use self::assets::Assets;
use self::notifications::NotificationData;
use self::widgets::NavItem;
use self::wizard::{WizardPage, WizardState};
use crate::notecache::NoteCache;

pub fn run() -> Result<(), Error> {
    let icon_bytes = include_bytes!("../../../logo/gossip.png");
    let icon = image::load_from_memory(icon_bytes)?.to_rgba8();
    let (icon_width, icon_height) = icon.dimensions();
    let icon = std::sync::Arc::new(IconData {
        rgba: icon.into_raw(),
        width: icon_width,
        height: icon_height,
    });

    let viewport = ViewportBuilder {
        title: Some("Gossip".to_string()),
        #[cfg(target_os = "linux")]
        app_id: Some("gossip".to_string()),
        min_inner_size: Some(egui::vec2(800.0, 600.0)),
        resizable: Some(true),
        decorations: Some(true),
        icon: Some(icon),
        #[cfg(target_os = "macos")]
        fullsize_content_view: Some(true),
        #[cfg(target_os = "macos")]
        titlebar_shown: Some(false),
        #[cfg(target_os = "macos")]
        title_shown: Some(false),
        drag_and_drop: Some(true),
        ..Default::default()
    };

    // According to eframe docs, we just need the directory:
    let mut persistence_path = gossip_lib::Profile::base_dir()?;
    // But based on testing, we need to give it the file:
    persistence_path.push("app.ron");

    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        vsync: true,
        renderer: if read_setting!(wgpu_renderer) {
            eframe::Renderer::Wgpu
        } else {
            eframe::Renderer::Glow
        },
        persist_window: true,
        persistence_path: Some(persistence_path),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "gossip",
        options,
        Box::new(|cc| Ok(Box::new(GossipUi::new(cc)))),
    ) {
        tracing::error!("Eframe error: {}", e);
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
enum Page {
    DmChatList,
    Feed(FeedKind),
    HandlerKinds,
    Handlers(EventKind),
    Notifications,
    PeopleLists,
    PeopleList(PersonList),
    Person(PublicKey),
    PersonFollows(PublicKey),
    PersonFollowers(PublicKey),
    YourKeys,
    YourMetadata,
    YourDelegation,
    YourNostrConnect,
    RelaysActivityMonitor,
    RelaysCoverage,
    RelaysMine,
    RelaysKnownNetwork(Option<RelayUrl>),
    SearchLocal,
    SearchRelays,
    Settings,
    HelpHelp,
    HelpStats,
    HelpAbout,
    #[allow(unused)]
    ThemeTest,
    Wizard(WizardPage),
}

impl Page {
    pub fn show_post_icon(&self) -> bool {
        use Page::*;
        #[allow(clippy::match_like_matches_macro)] // because we will add more later
        match *self {
            Feed(_) => true,
            _ => false,
        }
    }
}

impl Page {
    pub fn to_readable(&self) -> (&'static str /* Category */, String /* Name */) {
        match self {
            Page::DmChatList => (SubMenu::Feeds.as_str(), "Private msgs".into()),
            Page::Feed(feedkind) => ("Feed", feedkind.to_string()),
            Page::HandlerKinds => ("Event Handlers", "Event Handlers".into()),
            Page::Handlers(kind) => ("Event Handler", format!("{:?}", kind)),
            Page::Notifications => ("Notifications", "Notifications".into()),
            Page::PeopleLists => ("Lists", "Lists".into()),
            Page::PeopleList(list) => {
                let metadata = GLOBALS
                    .db()
                    .get_person_list_metadata(*list)
                    .unwrap_or_default()
                    .unwrap_or_default();
                ("Lists", metadata.title)
            }
            Page::Person(pk) => {
                let name = gossip_lib::names::best_name_from_pubkey_lookup(pk);
                ("Profile", name)
            }
            Page::PersonFollows(pk) => {
                let name = gossip_lib::names::best_name_from_pubkey_lookup(pk);
                ("Follows", name)
            }
            Page::PersonFollowers(pk) => {
                let name = gossip_lib::names::best_name_from_pubkey_lookup(pk);
                ("Followers", name)
            }
            Page::YourKeys => (SubMenu::Account.as_str(), "Keys".into()),
            Page::YourMetadata => (SubMenu::Account.as_str(), "Profile".into()),
            Page::YourDelegation => (SubMenu::Account.as_str(), "Delegation".into()),
            Page::YourNostrConnect => (SubMenu::Account.as_str(), "Nostr Connect".into()),
            Page::RelaysActivityMonitor => (SubMenu::Relays.as_str(), "Active Relays".into()),
            Page::RelaysCoverage => (SubMenu::Relays.as_str(), "Coverage Report".into()),
            Page::RelaysMine => (SubMenu::Relays.as_str(), "My Relays".into()),
            Page::RelaysKnownNetwork(_) => (SubMenu::Relays.as_str(), "Known Network".into()),
            Page::SearchLocal => ("Search Local", "Search Local".into()),
            Page::SearchRelays => ("Search Relays", "Search Relays".into()),
            Page::Settings => ("Settings", "Settings".into()),
            Page::HelpHelp => (SubMenu::Help.as_str(), "Troubleshooting".into()),
            Page::HelpStats => (SubMenu::Help.as_str(), "Stats".into()),
            Page::HelpAbout => (SubMenu::Help.as_str(), "About".into()),
            Page::ThemeTest => (SubMenu::Help.as_str(), "Theme Test".into()),
            Page::Wizard(wp) => ("Wizard", wp.as_str().to_string()),
        }
    }

    pub fn name(&self) -> String {
        self.to_readable().1
    }

    /* short string is used by back button hover text */
    fn to_short_string(&self) -> String {
        fn cat_name(page: &Page) -> String {
            let (cat, name) = page.to_readable();
            format!("{} {}", cat, name)
        }

        fn name_cat(page: &Page) -> String {
            let (cat, name) = page.to_readable();
            format!("{} {}", name, cat)
        }

        fn name(page: &Page) -> String {
            page.to_readable().1
        }

        match self {
            Page::DmChatList => cat_name(self),
            Page::Feed(_) => name_cat(self),
            Page::PeopleLists | Page::PeopleList(_) => cat_name(self),
            Page::Person(_) => name_cat(self),
            Page::PersonFollows(_) => name_cat(self),
            Page::PersonFollowers(_) => name_cat(self),
            Page::YourKeys | Page::YourMetadata | Page::YourDelegation | Page::YourNostrConnect => {
                cat_name(self)
            }
            Page::Wizard(_) => name_cat(self),
            _ => name(self),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SubMenu {
    Feeds,
    Search,
    Relays,
    Account,
    Help,
}

impl SubMenu {
    fn as_str(&self) -> &'static str {
        match self {
            SubMenu::Feeds => "Feeds",
            SubMenu::Search => "Search",
            SubMenu::Relays => "Relays",
            SubMenu::Account => "Account",
            SubMenu::Help => "Help",
        }
    }

    fn as_id_str(&self) -> &'static str {
        match self {
            SubMenu::Feeds => "feeds_submenu_id",
            SubMenu::Search => "search_submenu_id",
            SubMenu::Account => "account_submenu_id",
            SubMenu::Relays => "relays_submenu_id",
            SubMenu::Help => "help_submenu_id",
        }
    }
}

// this provides to_string(), implemented to make clipy happy
impl std::fmt::Display for SubMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Eq, Hash, PartialEq)]
enum SettingsTab {
    Content,
    Database,
    Id,
    Network,
    Posting,
    Ui,
}

pub enum HighlightType {
    Nothing,
    PublicKey,
    Event,
    Relay,
    Hyperlink,
}

#[derive(Debug, Clone)]
pub struct DraftData {
    // The draft text displayed in the edit textbox
    pub draft: String,

    // raw text output
    pub raw: String,

    // The last position of the TextEdit
    pub last_textedit_rect: Rect,
    pub is_more_menu_open: bool,

    // text replacements like nurls, hyperlinks or hashtags
    pub replacements: HashMap<String, ContentSegment>,
    pub replacements_changed: bool,

    pub include_subject: bool,
    pub subject: String,

    pub include_content_warning: bool,
    pub content_warning: String,

    // Data for normal draft
    pub repost: Option<Id>,
    pub replying_to: Option<Id>,

    // Are you sure
    pub are_you_sure_cancel: bool, // true if we are asking

    // If the user is typing a @tag, this is what they typed
    pub tagging_search_substring: Option<String>,
    pub tagging_search_selected: Option<usize>,
    pub tagging_search_searched: Option<String>,
    pub tagging_search_results: Vec<(String, PublicKey)>,

    // If this is an annotation
    pub is_annotate: bool,
}

impl Default for DraftData {
    fn default() -> DraftData {
        DraftData {
            draft: "".to_owned(),
            raw: "".to_owned(),
            last_textedit_rect: Rect::ZERO,
            is_more_menu_open: false,
            replacements: HashMap::new(),
            replacements_changed: false,
            include_subject: false,
            subject: "".to_owned(),
            include_content_warning: false,
            content_warning: "".to_owned(),

            // The following are ignored for DMs
            repost: None,
            replying_to: None,

            are_you_sure_cancel: false,

            tagging_search_substring: None,
            tagging_search_selected: None,
            tagging_search_searched: None,
            tagging_search_results: Vec::new(),

            is_annotate: false,
        }
    }
}

impl DraftData {
    pub fn clear(&mut self) {
        self.draft = "".to_owned();
        self.raw = "".to_owned();
        self.last_textedit_rect = Rect::ZERO;
        self.is_more_menu_open = false;
        self.replacements.clear();
        self.replacements_changed = true;
        self.include_subject = false;
        self.subject = "".to_owned();
        self.include_content_warning = false;
        self.content_warning = "".to_owned();
        self.repost = None;
        self.replying_to = None;
        self.are_you_sure_cancel = false;
        self.tagging_search_substring = None;
        self.tagging_search_selected = None;
        self.tagging_search_searched = None;
        self.tagging_search_results.clear();
        self.is_annotate = false;
    }
}

struct GossipUi {
    #[cfg(feature = "video-ffmpeg")]
    audio_device: Option<AudioDevice>,
    #[cfg(feature = "video-ffmpeg")]
    video_players: HashMap<Url, Rc<RefCell<egui_video::Player>>>,
    // dismissed libsdl2 warning
    #[cfg(feature = "video-ffmpeg")]
    warn_no_libsdl2_dismissed: bool,

    // Rendering
    next_frame: Instant,
    override_dpi: bool,
    override_dpi_value: u32,
    original_dpi_value: u32,
    // How far we should scroll this frame
    current_scroll_offset: f32,
    // How far we should scroll in future frames
    future_scroll_offset: f32,
    frame_count: usize,

    // Ui timers
    popups: HashMap<egui::Id, HashMap<egui::Id, Box<dyn widgets::InformationPopup>>>,

    // Modal dialogue
    modal: Option<Rc<ModalEntry>>,

    // QR codes being rendered (in feed or elsewhere)
    // the f32's are the recommended image size
    qr_codes: HashMap<String, Result<(TextureHandle, f32, f32), Error>>,

    // Processed events caching
    notecache: NoteCache,
    notification_data: NotificationData,

    // RelayUi
    relays: relays::RelayUi,

    // people::ListUi
    people_list: people::ListUi,

    // Handlers Ui
    handlers: Handlers,

    // Post rendering
    render_raw: Option<(Id, String)>,
    render_qr: Option<Id>,
    approved: HashSet<Id>, // content warning posts
    feed_note_height: HashMap<Id, f32>,

    // Person page rendering ('npub', 'nprofile', or 'lud06')
    person_qr: Option<&'static str>,
    setting_active_person: bool,

    // Page
    page: Page,
    history: Vec<Page>,
    submenu_ids: HashMap<SubMenu, egui::Id>,
    settings_tab: SettingsTab,

    // Feeds
    feeds: feed::Feeds,
    global_relays: Vec<String>,
    displayed_feed: Vec<Id>,
    displayed_feed_hash: Option<Id>,
    update_once_immediately: bool,

    // General Data
    assets: Assets,
    about: About,
    icon: TextureHandle,
    placeholder_avatar: TextureHandle,
    unsaved_settings: UnsavedSettings,
    theme: Theme,
    avatars: HashMap<PublicKey, TextureHandle>,
    images: HashMap<Url, TextureHandle>,
    blurs: HashMap<Url, TextureHandle>,
    /// used when settings.show_media=false to explicitly show
    media_show_list: HashSet<Url>,
    /// used when settings.show_media=true to explicitly hide
    media_hide_list: HashSet<Url>,
    /// media that the user has selected to show full-width
    media_full_width_list: HashSet<Url>,

    // User entry: posts
    show_post_area: bool,
    draft_needs_focus: bool,
    unlock_needs_focus: bool,
    draft_data: DraftData,
    previous_draft_data: DraftData,
    dm_draft_data: DraftData,
    dm_draft_data_target: Option<DmChannel>,

    // User entry: metadata
    editing_metadata: bool,
    metadata: Metadata,

    // User entry: delegatee tag (as JSON string)
    delegatee_tag_str: String,

    // User entry: general
    add_contact: String,
    password: String,
    password2: String,
    password3: String,
    delete_confirm: bool,
    new_metadata_fieldname: String,
    import_priv: String,
    import_pub: String,
    search: String,
    entering_a_search_page: bool,
    search_started: bool,
    editing_petname: bool,
    petname: String,
    deleting_list: Option<PersonList>,
    creating_list: bool,
    list_name_field_needs_focus: bool,
    new_list_name: String,
    new_list_favorite: bool,
    renaming_list: Option<PersonList>,
    editing_list_error: Option<String>,
    nostr_connect_name: String,
    nostr_connect_relay1: String,
    nostr_connect_relay2: String,

    // search result
    search_note_height: HashMap<Id, f32>,
    search_person_height: HashMap<PublicKey, f32>,

    // Collapsed threads
    collapsed: Vec<Id>,

    // Fully opened posts
    opened: HashSet<Id>,

    // Visible Note IDs
    // (we resubscribe to reactions/zaps/deletes when this changes)
    visible_note_ids: Vec<Id>,
    // This one is built up as rendering happens, then compared
    next_visible_note_ids: Vec<Id>,
    last_visible_update: Instant,

    note_showing_reactions: Option<Id>,

    // Zap state, computed once per frame instead of per note
    // zap_state and note_being_zapped are computed from GLOBALS.current_zap and are
    //   not authoritative.
    zap_state: ZapState,
    note_being_zapped: Option<Id>,
    note_showing_zaps: Option<Id>,
    zap_amount_input: u64,

    wizard_state: WizardState,

    theme_test: crate::ui::theme::test_page::ThemeTest,

    // Cached DM Channels
    dm_channel_cache: Vec<DmChannelData>,
    dm_channel_next_refresh: Instant,
    dm_channel_error: Option<String>,

    file_dialog: FileDialog,
    uploading: Option<PathBuf>,
}

impl Drop for GossipUi {
    fn drop(&mut self) {
        self.password.zeroize();
        self.password2.zeroize();
        self.password3.zeroize();
    }
}

impl GossipUi {
    fn new(cctx: &eframe::CreationContext<'_>) -> Self {
        {
            cctx.egui_ctx.tessellation_options_mut(|to| {
                // Less feathering
                to.feathering = true;
                to.feathering_size_in_pixels = 0.667;

                // Sharper text
                to.round_text_to_pixels = true;
            });

            cctx.egui_ctx.options_mut(|o| {
                o.theme_preference = if read_setting!(follow_os_dark_mode) {
                    egui::ThemePreference::System
                } else if read_setting!(dark_mode) {
                    egui::ThemePreference::Dark
                } else {
                    egui::ThemePreference::Light
                }
            });
        }

        let mut submenu_ids: HashMap<SubMenu, egui::Id> = HashMap::new();
        submenu_ids.insert(SubMenu::Feeds, egui::Id::new(SubMenu::Feeds.as_id_str()));
        submenu_ids.insert(SubMenu::Search, egui::Id::new(SubMenu::Search.as_id_str()));
        submenu_ids.insert(
            SubMenu::Account,
            egui::Id::new(SubMenu::Account.as_id_str()),
        );
        submenu_ids.insert(SubMenu::Relays, egui::Id::new(SubMenu::Relays.as_id_str()));
        submenu_ids.insert(SubMenu::Help, egui::Id::new(SubMenu::Help.as_id_str()));

        // load Assets, but load again when DPI changes
        let assets = Assets::init(&cctx.egui_ctx);

        let icon_texture_handle = {
            let bytes = include_bytes!("../../../logo/gossip.png");
            let image = image::load_from_memory(bytes).unwrap();
            let size = [image.width() as _, image.height() as _];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            cctx.egui_ctx.load_texture(
                "icon",
                ImageData::Color(
                    ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()).into(),
                ),
                TextureOptions::default(), // magnification, minification
            )
        };

        let placeholder_avatar_texture_handle = {
            let bytes = include_bytes!("../../../assets/placeholder_avatar.png");
            let image = image::load_from_memory(bytes).unwrap();
            let size = [image.width() as _, image.height() as _];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            cctx.egui_ctx.load_texture(
                "placeholder_avatar",
                ImageData::Color(
                    ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()).into(),
                ),
                TextureOptions::default(), // magnification, minification
            )
        };

        #[cfg(feature = "video-ffmpeg")]
        let audio_device = {
            let mut device = None;
            if let Ok(init) = sdl2::init() {
                if let Ok(audio) = init.audio() {
                    if let Ok(dev) = egui_video::init_audio_device(&audio) {
                        device = Some(dev);
                    }
                }
            }
            device
        };

        // start with a fallback DPI here, unless we are overriding anyways
        // we won't know the native DPI until the `Viewport` has been created
        let (override_dpi, override_dpi_value): (bool, u32) = match read_setting!(override_dpi) {
            Some(v) => (true, v),
            None => (false, (cctx.egui_ctx.pixels_per_point() * 72.0) as u32),
        };

        let include_all = read_feed_include_all(
            &FeedKind::List(PersonList::Followed, true), // 'true/false' isn't considered
            &cctx.egui_ctx,
        );
        let mut start_page = Page::Feed(FeedKind::List(PersonList::Followed, include_all));

        // Possibly enter the wizard instead
        let mut wizard_state: WizardState = Default::default();
        let wizard_complete = GLOBALS.db().get_flag_wizard_complete();
        if !wizard_complete {
            wizard_state.init();
            if let Some(wp) = wizard::start_wizard_page(&mut wizard_state) {
                start_page = Page::Wizard(wp);
            }
        }

        // Honor sys dark mode, if set
        if read_setting!(follow_os_dark_mode) {
            let sys_dark_mode = cctx.egui_ctx.style().visuals.dark_mode;
            if read_setting!(dark_mode) != sys_dark_mode {
                write_setting!(dark_mode, sys_dark_mode);
            }
        }

        // Apply current theme
        let theme = Theme::from_settings();
        theme::apply_theme(&theme, &cctx.egui_ctx);

        // Let gossip-lib know the max texture side so it can resize things that are
        // too large.
        {
            let mut max_image_side = if let Some(context) = &cctx.gl {
                use eframe::glow::HasContext;
                unsafe { context.get_parameter_i32(eframe::glow::MAX_TEXTURE_SIZE) as usize }
            } else {
                2048
            };
            if max_image_side > 16384 {
                max_image_side = 16384;
            }
            GLOBALS
                .max_image_side
                .store(max_image_side, Ordering::Relaxed);
        }

        // Start a thread that listens on GLOBALS.notify_ui_redraw and informs
        // the Context to redraw.  This can't be done in update() because update()
        // is only run periodically.
        let copy_context = cctx.egui_ctx.clone();
        GLOBALS.runtime.spawn(async move {
            loop {
                GLOBALS.notify_ui_redraw.notified().await;
                copy_context.request_repaint();
            }
        });

        GossipUi {
            #[cfg(feature = "video-ffmpeg")]
            audio_device,
            #[cfg(feature = "video-ffmpeg")]
            video_players: HashMap::new(),
            #[cfg(feature = "video-ffmpeg")]
            warn_no_libsdl2_dismissed: false,
            next_frame: Instant::now(),
            override_dpi,
            override_dpi_value,
            original_dpi_value: override_dpi_value,
            current_scroll_offset: 0.0,
            future_scroll_offset: 0.0,
            frame_count: 0,
            popups: HashMap::new(),
            modal: None,
            qr_codes: HashMap::new(),
            notecache: NoteCache::new(),
            notification_data: NotificationData::new(),
            relays: relays::RelayUi::new(),
            people_list: people::ListUi::new(),
            handlers: Default::default(),
            render_raw: None,
            render_qr: None,
            approved: HashSet::new(),
            feed_note_height: HashMap::new(),
            person_qr: None,
            setting_active_person: false,
            page: start_page,
            history: vec![],
            submenu_ids,
            settings_tab: SettingsTab::Id,
            feeds: feed::Feeds::default(),
            global_relays: Vec::new(),
            displayed_feed: Vec::new(),
            displayed_feed_hash: None,
            update_once_immediately: false,

            // load Assets, but load again when DPI changes
            assets,
            about: About::new(),
            icon: icon_texture_handle,
            placeholder_avatar: placeholder_avatar_texture_handle,
            unsaved_settings: UnsavedSettings::load(),
            theme,
            avatars: HashMap::new(),
            images: HashMap::new(),
            blurs: HashMap::new(),
            media_show_list: HashSet::new(),
            media_hide_list: HashSet::new(),
            media_full_width_list: HashSet::new(),
            show_post_area: false,
            draft_needs_focus: false,
            unlock_needs_focus: true,
            draft_data: DraftData::default(),
            previous_draft_data: DraftData::default(),
            dm_draft_data: DraftData::default(),
            dm_draft_data_target: None,
            editing_metadata: false,
            metadata: Metadata::new(),
            delegatee_tag_str: "".to_owned(),
            add_contact: "".to_owned(),
            password: "".to_owned(),
            password2: "".to_owned(),
            password3: "".to_owned(),
            delete_confirm: false,
            new_metadata_fieldname: String::new(),
            import_priv: "".to_owned(),
            import_pub: "".to_owned(),
            search: "".to_owned(),
            entering_a_search_page: false,
            search_started: false,
            editing_petname: false,
            petname: "".to_owned(),
            deleting_list: None,
            creating_list: false,
            list_name_field_needs_focus: false,
            new_list_name: "".to_owned(),
            new_list_favorite: false,
            renaming_list: None,
            editing_list_error: None,
            nostr_connect_name: "".to_owned(),
            nostr_connect_relay1: "".to_owned(),
            nostr_connect_relay2: "".to_owned(),
            search_note_height: HashMap::new(),
            search_person_height: HashMap::new(),
            collapsed: vec![],
            opened: HashSet::new(),
            visible_note_ids: vec![],
            next_visible_note_ids: vec![],
            last_visible_update: Instant::now(),
            note_showing_reactions: None,
            zap_state: ZapState::None,
            note_being_zapped: None,
            note_showing_zaps: None,
            zap_amount_input: 10,
            wizard_state,
            theme_test: Default::default(),
            dm_channel_cache: vec![],
            dm_channel_next_refresh: Instant::now(),
            dm_channel_error: None,
            file_dialog: FileDialog::new(),
            uploading: None,
        }
    }

    /// Since egui 0.24 "multi viewport" this function needs
    /// to be called on the first App::update() because the
    /// native PPT is None until the Viewport is created
    fn init_scaling(&mut self, ctx: &Context) {
        (self.override_dpi, self.override_dpi_value) =
            if let Some(override_dpi) = read_setting!(override_dpi) {
                let ppt: f32 = override_dpi as f32 / 72.0;
                ctx.set_pixels_per_point(ppt);
                let dpi = (ppt * 72.0) as u32;
                tracing::info!("DPI (overridden): {}", dpi);
                (true, dpi)
            } else if let Some(ppt) = ctx.native_pixels_per_point() {
                ctx.set_pixels_per_point(ppt);
                let dpi = (ppt * 72.0) as u32;
                tracing::info!("DPI (native): {}", dpi);
                (false, dpi)
            } else {
                let dpi = (ctx.pixels_per_point() * 72.0) as u32;
                tracing::info!("DPI (fallback): {}", dpi);
                (false, dpi)
            };

        // 'original' refers to 'before the user changes it in settings'
        self.original_dpi_value = self.override_dpi_value;

        // Reload Assets when DPI changes
        self.assets = Assets::init(ctx);

        // Set global pixels_per_point_times_100, used for image scaling.
        // this would warrant reloading images but the user experience isn't great as
        // reloading them takes quite a while currently
        GLOBALS
            .pixels_per_point_times_100
            .store((ctx.pixels_per_point() * 100.0) as u32, Ordering::Relaxed);
    }

    // maybe_relays is only used for Page::Feed(FeedKind::Thread...)
    fn set_page(&mut self, ctx: &Context, page: Page) {
        if self.page != page {
            let within_wizard =
                matches!(self.page, Page::Wizard(_)) && matches!(page, Page::Wizard(_));

            if within_wizard {
                // Within the wizard we don't need to do history or
                // special handling. But we do need to fall through
                // to clearing passwords.
                self.page = page;
            } else {
                tracing::trace!("PUSHING HISTORY: {:?}", &self.page);
                self.history.push(self.page.clone());
                self.set_page_inner(ctx, page);
            }

            // Clear QR codes on page switches
            self.qr_codes.clear();
            self.render_qr = None;
            self.person_qr = None;

            // Clear sensitive fields on page switches
            self.password.zeroize();
            self.password = "".to_owned();
            self.password2.zeroize();
            self.password2 = "".to_owned();
            self.password3.zeroize();
            self.password3 = "".to_owned();
            self.import_priv.zeroize();
            self.import_priv = "".to_owned();
        }
    }

    fn back(&mut self, ctx: &Context) {
        if let Some(page) = self.history.pop() {
            tracing::trace!("POPPING HISTORY: {:?}", &page);
            self.set_page_inner(ctx, page);
        } else {
            tracing::trace!("HISTORY STUCK ON NONE");
        }
    }

    fn set_page_inner(&mut self, ctx: &Context, page: Page) {
        // Setting the page often requires some associated actions:
        match &page {
            Page::Feed(feed_kind) => {
                let is_list = matches!(feed_kind, FeedKind::List(_, _));
                GLOBALS.feed.switch_feed(feed_kind.clone());
                feed::enter_feed(self, ctx, feed_kind.to_owned());
                if is_list {
                    self.open_menu(ctx, SubMenu::Feeds);
                } else {
                    self.close_all_menus_except_feeds(ctx);
                }
            }
            Page::PeopleLists => {
                people::enter_page(self);
                self.close_all_menus_except_feeds(ctx);
            }
            Page::Person(pubkey) => {
                self.close_all_menus_except_feeds(ctx);
                // Fetch metadata for that person at the page switch
                // (this bypasses checking if it was done recently)
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::UpdateMetadata(*pubkey));
            }
            Page::PersonFollows(pubkey) => {
                self.close_all_menus_except_feeds(ctx);

                if GLOBALS.follows.read().who != Some(*pubkey) {
                    // Switch tracking to them
                    GLOBALS.follows.write().reset(*pubkey);

                    // Initiate tracking followed
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::TrackFollows(*pubkey));
                }
            }
            Page::PersonFollowers(pubkey) => {
                self.close_all_menus_except_feeds(ctx);

                if GLOBALS.followers.read().who != Some(*pubkey) {
                    // Switch tracking to them
                    GLOBALS.followers.write().reset(*pubkey);

                    // Initiate pulling of contact lists referencing them
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::TrackFollowers(*pubkey));
                }
            }
            Page::YourKeys | Page::YourMetadata | Page::YourDelegation | Page::YourNostrConnect => {
                self.open_menu(ctx, SubMenu::Account);
            }
            Page::RelaysActivityMonitor | Page::RelaysCoverage | Page::RelaysMine => {
                self.relays.enter_page(None);
                self.open_menu(ctx, SubMenu::Relays);
            }
            Page::RelaysKnownNetwork(some_relay) => {
                self.relays.enter_page(some_relay.as_ref());
                self.open_menu(ctx, SubMenu::Relays);
            }
            Page::SearchLocal => {
                self.entering_a_search_page = true;
                self.search_started = false;
                self.open_menu(ctx, SubMenu::Search);
            }
            Page::SearchRelays => {
                self.entering_a_search_page = true;
                self.search_started = false;
                self.open_menu(ctx, SubMenu::Search);
            }
            Page::Settings => {
                self.close_all_menus_except_feeds(ctx);
            }
            Page::HelpHelp | Page::HelpStats | Page::HelpAbout => {
                self.open_menu(ctx, SubMenu::Help);
            }
            Page::Notifications => {
                let _ = GLOBALS.pending.compute_pending();
                self.close_all_menus_except_feeds(ctx);
            }
            _ => {
                self.close_all_menus_except_feeds(ctx);
            }
        }

        // clear some search state
        GLOBALS.events_being_searched_for.write().clear();

        self.page = page;
    }

    fn bottom_panel(&mut self, ctx: &Context) {
        egui::TopBottomPanel::bottom("notification-panel")
            .show_separator_line(true)
            .frame(
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(0.0, 0.0))
                    .fill(self.theme.navigation_bg_fill()),
            )
            .show(ctx, |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    notifications::draw_icons(self, ui);
                    if read_setting!(frame_spinner) {
                        ui.add_space(2.0);
                        self.render_spinner(ui);
                        ui.add_space(8.0);
                    } else {
                        ui.add_space(10.0);
                    }
                    self.render_status_queue_area(ui);
                    ui.add_space(10.0);
                    self.add_offline_switch(ui);
                });
            });
    }

    fn side_panel(&mut self, ctx: &Context) {
        egui::SidePanel::left("main-navigation-panel")
            .show_separator_line(false)
            .frame(
                egui::Frame::none()
                    .inner_margin({
                        #[cfg(not(target_os = "macos"))]
                        let margin = egui::Margin::symmetric(20.0, 20.0);
                        #[cfg(target_os = "macos")]
                        let margin = egui::Margin {
                            left: 20.0,
                            right: 20.0,
                            top: 35.0,
                            bottom: 20.0,
                        };
                        margin
                    })
                    .fill(self.theme.navigation_bg_fill()),
            )
            .show(ctx, |ui| {
                self.begin_ui(ui);

                // Cut indentation
                ui.style_mut().spacing.indent = 0.0;
                ui.visuals_mut().widgets.inactive.fg_stroke.color =
                    self.theme.navigation_text_color();
                ui.visuals_mut().widgets.hovered.fg_stroke.color =
                    self.theme.navigation_text_hover_color();
                ui.visuals_mut().widgets.hovered.fg_stroke.width = 1.0;
                ui.visuals_mut().widgets.active.fg_stroke.color =
                    self.theme.navigation_text_active_color();

                ui.add_space(4.0);
                self.add_back_button(ui, ctx);

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                self.add_feeds_submenu(ui, ctx);
                self.add_global_feed(ui, ctx);
                self.add_personal_notes(ui, ctx);
                self.add_private_chats(ui, ctx);
                self.add_search_submenu(ui, ctx);

                ui.add_space(10.0);

                self.add_people_lists(ui, ctx);
                self.add_relays_submenu(ui, ctx);
                self.add_account_submenu(ui, ctx);
                self.add_handlers(ui, ctx);
                self.add_settings(ui, ctx);
                self.add_help_submenu(ui, ctx);

                #[cfg(debug_assertions)]
                self.add_theme_test(ui, ctx);

                self.add_debug_area(ui);
                self.add_plus_icon(ui, ctx);
            });
    }

    fn add_back_button(&mut self, ui: &mut Ui, ctx: &Context) {
        let back_label_text = RichText::new("‹ Back");
        let label = if self.history.is_empty() {
            Label::new(back_label_text.color(self.theme.navigation_text_deactivated_color()))
        } else {
            Label::new(back_label_text.color(self.theme.navigation_text_color()))
                .sense(Sense::click())
        };
        let response = ui.add(label);
        let response = if let Some(page) = self.history.last() {
            response.on_hover_text(format!("back to {}", page.to_short_string()))
        } else {
            response
        };
        let response = if !self.history.is_empty() {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response.on_hover_cursor(egui::CursorIcon::NotAllowed)
        };
        if response.clicked() {
            self.back(ctx);
        }
    }

    fn add_feeds_submenu(&mut self, ui: &mut Ui, ctx: &Context) {
        let (mut cstate, header_response) = self.get_openable_menu(ui, ctx, SubMenu::Feeds);
        cstate.show_body_indented(&header_response, ui, |ui| {
            let mut all_lists = GLOBALS
                .db()
                .get_all_person_list_metadata()
                .unwrap_or_default();
            all_lists.sort_by(people::sort_lists);

            let mut more: usize = 0;
            for (list, metadata) in all_lists {
                if list == PersonList::Muted {
                    more += 1;
                    continue;
                }
                if list == PersonList::Followed || metadata.favorite {
                    let include_all = read_feed_include_all(
                        &FeedKind::List(list, true), // 'true/false' isn't considered
                        ctx,
                    );
                    self.add_menu_item_page(
                        ui,
                        Page::Feed(FeedKind::List(list, include_all)),
                        Some(&metadata.title),
                        true,
                    );
                } else {
                    more += 1;
                }
            }
            if more != 0 {
                self.add_menu_item_page(
                    ui,
                    Page::PeopleLists,
                    Some(&format!("More ({})...", more)),
                    false, // do not highlight this entry
                );
            }
        });
        self.after_openable_menu(ui, &cstate);
    }

    fn add_global_feed(&mut self, ui: &mut Ui, ctx: &Context) {
        if self
            .add_selected_label(ui, self.page == Page::Feed(FeedKind::Global), "Global")
            .clicked()
        {
            self.set_page(ctx, Page::Feed(FeedKind::Global));
        }
    }

    fn add_personal_notes(&mut self, ui: &mut Ui, ctx: &Context) {
        if let Some(pubkey) = GLOBALS.identity.public_key() {
            if self
                .add_selected_label(
                    ui,
                    self.page == Page::Feed(FeedKind::Person(pubkey)),
                    "My notes",
                )
                .clicked()
            {
                self.set_page(ctx, Page::Feed(FeedKind::Person(pubkey)));
            }

            let include_all = read_feed_include_all(
                &FeedKind::Inbox(true), // 'true/false' isn't considered
                ctx,
            );
            let response = self.add_selected_label(
                ui,
                self.page == Page::Feed(FeedKind::Inbox(include_all)),
                "Inbox",
            );
            if response.clicked() {
                self.set_page(ctx, Page::Feed(FeedKind::Inbox(include_all)));
            }

            // Add unread messages indicator for inbox
            let count = GLOBALS.unread_inbox.load(Ordering::Relaxed);
            if count > 0 {
                let where_to_put_background = ui.painter().add(egui::Shape::Noop);
                let pos = response.rect.right_center() + vec2(10.0, 1.0);
                let text_color = if self.theme.dark_mode {
                    self.theme.neutral_900()
                } else {
                    self.theme.neutral_100()
                };
                let bg_rect = ui
                    .painter()
                    .text(
                        pos,
                        egui::Align2::LEFT_CENTER,
                        format!("{}", count),
                        FontId::proportional(8.0),
                        text_color,
                    )
                    .expand2(vec2(5.0, 2.0))
                    .translate(vec2(0.0, -1.0)); // FIXME: Hack to fix the line height
                let bg_shape = egui::Shape::rect_filled(
                    bg_rect,
                    egui::Rounding::same(bg_rect.height()),
                    self.theme.accent_color(),
                );
                ui.painter().set(where_to_put_background, bg_shape);
            }

            if self
                .add_selected_label(
                    ui,
                    self.page == Page::Feed(FeedKind::Bookmarks),
                    "Bookmarks",
                )
                .clicked()
            {
                self.set_page(ctx, Page::Feed(FeedKind::Bookmarks));
            }
        }
    }

    fn add_private_chats(&mut self, ui: &mut Ui, ctx: &Context) {
        if GLOBALS.identity.is_unlocked() {
            let response =
                self.add_selected_label(ui, self.page == Page::DmChatList, "Private chats");
            if response.clicked() {
                self.set_page(ctx, Page::DmChatList);
            }

            // Add unread messages indicator
            let count = GLOBALS.unread_dms.load(Ordering::Relaxed);
            if count > 0 {
                let where_to_put_background = ui.painter().add(egui::Shape::Noop);
                let pos = response.rect.right_center() + vec2(10.0, 1.0);
                let text_color = if self.theme.dark_mode {
                    self.theme.neutral_900()
                } else {
                    self.theme.neutral_100()
                };
                let bg_rect = ui
                    .painter()
                    .text(
                        pos,
                        egui::Align2::LEFT_CENTER,
                        format!("{}", count),
                        FontId::proportional(8.0),
                        text_color,
                    )
                    .expand2(vec2(5.0, 2.0))
                    .translate(vec2(0.0, -1.0)); // FIXME: Hack to fix the line height
                let bg_shape = egui::Shape::rect_filled(
                    bg_rect,
                    egui::Rounding::same(bg_rect.height()),
                    self.theme.accent_color(),
                );
                ui.painter().set(where_to_put_background, bg_shape);
            }
        }
    }

    fn add_search_submenu(&mut self, ui: &mut Ui, ctx: &Context) {
        let (mut cstate, header_response) = self.get_openable_menu(ui, ctx, SubMenu::Search);
        cstate.show_body_indented(&header_response, ui, |ui| {
            self.add_menu_item_page(ui, Page::SearchLocal, None, true);
            self.add_menu_item_page(ui, Page::SearchRelays, None, true);
        });
        self.after_openable_menu(ui, &cstate);
    }

    fn add_people_lists(&mut self, ui: &mut Ui, ctx: &Context) {
        if self
            .add_selected_label(ui, self.page == Page::PeopleLists, "People Lists")
            .clicked()
        {
            self.set_page(ctx, Page::PeopleLists);
        }
    }

    fn add_relays_submenu(&mut self, ui: &mut Ui, ctx: &Context) {
        let (mut cstate, header_response) = self.get_openable_menu(ui, ctx, SubMenu::Relays);
        cstate.show_body_indented(&header_response, ui, |ui| {
            self.add_menu_item_page(ui, Page::RelaysActivityMonitor, None, true);
            self.add_menu_item_page(ui, Page::RelaysMine, None, true);
            self.add_menu_item_page(ui, Page::RelaysKnownNetwork(None), None, true);
        });
        self.after_openable_menu(ui, &cstate);
    }

    fn add_account_submenu(&mut self, ui: &mut Ui, ctx: &Context) {
        let (mut cstate, header_response) = self.get_openable_menu(ui, ctx, SubMenu::Account);
        cstate.show_body_indented(&header_response, ui, |ui| {
            self.add_menu_item_page(ui, Page::YourMetadata, None, true);
            self.add_menu_item_page(ui, Page::YourKeys, None, true);
            self.add_menu_item_page(ui, Page::YourDelegation, None, true);
            self.add_menu_item_page(ui, Page::YourNostrConnect, None, true);
        });
        self.after_openable_menu(ui, &cstate);
    }

    fn add_handlers(&mut self, ui: &mut Ui, ctx: &Context) {
        if self
            .add_selected_label(
                ui,
                self.page == Page::HandlerKinds || matches!(self.page, Page::Handlers(_)),
                "Handlers",
            )
            .clicked()
        {
            self.set_page(ctx, Page::HandlerKinds);
        }
    }

    fn add_settings(&mut self, ui: &mut Ui, ctx: &Context) {
        if self
            .add_selected_label(ui, self.page == Page::Settings, "Settings")
            .clicked()
        {
            self.set_page(ctx, Page::Settings);
        }
    }

    fn add_help_submenu(&mut self, ui: &mut Ui, ctx: &Context) {
        let (mut cstate, header_response) = self.get_openable_menu(ui, ctx, SubMenu::Help);
        cstate.show_body_indented(&header_response, ui, |ui| {
            self.add_menu_item_page(ui, Page::HelpHelp, None, true);
            self.add_menu_item_page(ui, Page::HelpStats, None, true);
            self.add_menu_item_page(ui, Page::HelpAbout, None, true);
        });
        self.after_openable_menu(ui, &cstate);
    }

    #[cfg(debug_assertions)]
    fn add_theme_test(&mut self, ui: &mut Ui, ctx: &Context) {
        if self
            .add_selected_label(ui, self.page == Page::ThemeTest, "Theme Test")
            .clicked()
        {
            self.set_page(ctx, Page::ThemeTest);
        }
    }

    fn add_offline_switch(&mut self, ui: &mut Ui) {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (_frame_stroke, active_color_override) =
                if self.theme.dark_mode && self.unsaved_settings.offline {
                    (
                        egui::Stroke::new(1.0, Color32::TRANSPARENT),
                        Some(self.theme.neutral_900()),
                    )
                } else if self.theme.dark_mode && !self.unsaved_settings.offline {
                    (
                        egui::Stroke::new(1.0, self.theme.neutral_900()),
                        Some(self.theme.neutral_900()),
                    )
                } else if !self.theme.dark_mode && self.unsaved_settings.offline {
                    (egui::Stroke::new(1.0, Color32::TRANSPARENT), None)
                } else {
                    (egui::Stroke::new(1.0, self.theme.neutral_300()), None)
                };
            let (color, text, text_color_override) = if self.unsaved_settings.offline {
                (self.theme.amber_100(), "OFFLINE", active_color_override)
            } else {
                (
                    Color32::TRANSPARENT,
                    "Go offline",
                    Some(self.theme.neutral_500()),
                )
            };
            const PADDING: egui::Margin = egui::Margin {
                left: 16.0,
                right: 16.0,
                top: 5.0,
                bottom: 5.0,
            };
            egui::Frame::none()
                .rounding(egui::Rounding::ZERO)
                .outer_margin(egui::Margin {
                    left: 0.0,
                    right: 0.0,
                    top: 0.0,
                    bottom: 0.0,
                })
                .inner_margin(PADDING)
                .fill(color)
                .show(ui, |ui| {
                    ui.set_height(25.0);
                    ui.set_width(115.0);

                    // switch
                    let response =
                        widgets::Switch::small(&self.theme, &mut self.unsaved_settings.offline)
                            .with_label(text)
                            .with_label_color(text_color_override)
                            .show(ui);

                    if response.changed() {
                        if let Err(e) = self.unsaved_settings.save() {
                            tracing::error!("Error saving settings: {}", e);
                        }
                    }
                });
        });
    }

    fn add_debug_area(&mut self, ui: &mut Ui) {
        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            // DEBUG status area
            if read_setting!(status_bar) {
                let num_stalled = GLOBALS.fetcher.num_requests_stalled();
                let num_in_flight = GLOBALS.fetcher.num_requests_in_flight();

                let m = format!("HTTP: {} / {}", num_in_flight, num_stalled);
                ui.add(Label::new(
                    RichText::new(m).color(self.theme.notice_marker_text_color()),
                ));

                let subs = GLOBALS.open_subscriptions.load(Ordering::Relaxed);
                let m = format!("RS {}", subs);
                ui.add(Label::new(
                    RichText::new(m).color(self.theme.notice_marker_text_color()),
                ));

                let relays = GLOBALS.connected_relays.len();
                let m = format!("RC {}", relays);
                ui.add(Label::new(
                    RichText::new(m).color(self.theme.notice_marker_text_color()),
                ));

                let events = GLOBALS.db().get_event_len().unwrap_or(0);
                let m = format!("ES {}", events);
                ui.add(Label::new(
                    RichText::new(m).color(self.theme.notice_marker_text_color()),
                ));

                let processed = GLOBALS.events_processed.load(Ordering::Relaxed);
                let m = format!("EI {}", processed);
                ui.add(Label::new(
                    RichText::new(m).color(self.theme.notice_marker_text_color()),
                ));

                ui.separator();
            }
        });
    }

    fn add_plus_icon(&mut self, ui: &mut Ui, ctx: &Context) {
        if !self.show_post_area_fn() && self.page.show_post_icon() {
            let feed_newest_at_bottom = GLOBALS.db().read_setting_feed_newest_at_bottom();
            let pos = if feed_newest_at_bottom {
                let top_right = ui.ctx().screen_rect().right_top();
                top_right + Vec2::new(-crate::AVATAR_SIZE_F32 * 2.0, crate::AVATAR_SIZE_F32 * 2.0)
            } else {
                let bottom_right = ui.ctx().screen_rect().right_bottom();
                bottom_right
                    + Vec2::new(-crate::AVATAR_SIZE_F32 * 2.0, -crate::AVATAR_SIZE_F32 * 2.0)
            };

            egui::Area::new(ui.next_auto_id().with("plus"))
                .movable(false)
                .interactable(true)
                .fixed_pos(pos)
                .constrain(true)
                .show(ctx, |ui| {
                    self.begin_ui(ui);
                    egui::Frame::popup(&self.theme.get_style())
                        .rounding(egui::Rounding::same(crate::AVATAR_SIZE_F32 / 2.0)) // need the rounding for the shadow
                        .stroke(egui::Stroke::NONE)
                        .fill(Color32::TRANSPARENT)
                        .shadow(egui::epaint::Shadow::NONE)
                        .show(ui, |ui| {
                            let text = if GLOBALS.identity.is_unlocked() {
                                RichText::new("+").size(22.5)
                            } else {
                                RichText::new("\u{1f513}").size(20.0)
                            };
                            let fill_color = {
                                let fill_color_tuple = self.theme.accent_color().to_tuple();
                                Color32::from_rgba_premultiplied(
                                    fill_color_tuple.0,
                                    fill_color_tuple.1,
                                    fill_color_tuple.2,
                                    170, // 2/3 transparent
                                )
                            };
                            let response = ui.add_sized(
                                [crate::AVATAR_SIZE_F32, crate::AVATAR_SIZE_F32],
                                egui::Button::new(
                                    text.color(self.theme.get_style().visuals.panel_fill),
                                )
                                .stroke(egui::Stroke::NONE)
                                .rounding(egui::Rounding::same(crate::AVATAR_SIZE_F32))
                                .fill(fill_color),
                            );
                            if response.clicked() {
                                self.show_post_area = true;
                                if GLOBALS.identity.is_unlocked() {
                                    self.draft_needs_focus = true;
                                } else {
                                    self.unlock_needs_focus = true;
                                }
                            }
                            response.on_hover_cursor(egui::CursorIcon::PointingHand);
                        });
                });
        }
    }

    #[inline]
    fn is_scrolling(&self) -> bool {
        self.current_scroll_offset != 0.0
    }

    fn enable_ui(&self) -> bool {
        !relays::is_entry_dialog_active(self)
            && self.person_qr.is_none()
            && self.render_qr.is_none()
            && self.render_raw.is_none()
    }

    fn begin_ui(&self, ui: &mut Ui) {
        // if a dialog is open, disable the rest of the UI
        if !self.enable_ui() {
            ui.disable();
        }
    }

    pub fn richtext_from_person_nip05(person: &Person) -> RichText {
        if let Some(mut nip05) = person.nip05().map(|s| s.to_owned()) {
            if nip05.starts_with("_@") {
                nip05 = nip05.get(2..).unwrap().to_string();
            }

            if person.nip05_valid {
                RichText::new(nip05).monospace()
            } else {
                RichText::new(nip05).monospace().strikethrough()
            }
        } else {
            RichText::default()
        }
    }

    pub fn render_person_name_line(
        app: &mut GossipUi,
        ui: &mut Ui,
        person: &Person,
        profile_page: bool,
    ) {
        // Let the 'People' manager know that we are interested in displaying this person.
        // It will take all actions necessary to make the data eventually available.
        GLOBALS.people.person_of_interest(person.pubkey);

        ui.horizontal_wrapped(|ui| {
            let followed = person.is_in_list(PersonList::Followed);
            let muted = person.is_in_list(PersonList::Muted);
            let is_self = if let Some(pubkey) = GLOBALS.identity.public_key() {
                pubkey == person.pubkey
            } else {
                false
            };

            let tag_name_menu = {
                let text = if !profile_page {
                    person.best_name()
                } else {
                    "ACTIONS".to_string()
                };
                RichText::new(format!("☰ {}", text))
            };

            ui.menu_button(tag_name_menu, |ui| {
                if !profile_page {
                    if ui.button("View Person").clicked() {
                        app.set_page(ui.ctx(), Page::Person(person.pubkey));
                    }
                }
                if app.page != Page::Feed(FeedKind::Person(person.pubkey)) {
                    if ui.button("View Their Posts").clicked() {
                        app.set_page(ui.ctx(), Page::Feed(FeedKind::Person(person.pubkey)));
                    }
                }
                if GLOBALS.identity.is_unlocked() {
                    if ui.button("Send DM").clicked() {
                        let channel = DmChannel::new(&[person.pubkey]);
                        app.set_page(ui.ctx(), Page::Feed(FeedKind::DmChat(channel)));
                    }
                }
                if !followed && ui.button("Follow").clicked() {
                    let _ = GLOBALS.people.follow(
                        &person.pubkey,
                        true,
                        PersonList::Followed,
                        Private(false),
                    );
                } else if followed && ui.button("Unfollow").clicked() {
                    let _ = GLOBALS.people.follow(
                        &person.pubkey,
                        false,
                        PersonList::Followed,
                        Private(false),
                    );
                }

                // Do not show 'Mute' if this is yourself
                if muted || !is_self {
                    let mute_label = if muted { "Unmute" } else { "Mute" };
                    if ui.button(mute_label).clicked() {
                        let _ = GLOBALS.people.mute(&person.pubkey, !muted, Private(false));
                        app.notecache.invalidate_person(&person.pubkey);
                    }
                }

                if ui.button("Update Metadata").clicked() {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::UpdateMetadata(person.pubkey));
                }

                if ui.button("Copy web link").clicked() {
                    ui.output_mut(|o| {
                        let mut profile = Profile {
                            pubkey: person.pubkey,
                            relays: Vec::new(),
                        };
                        let relays = GLOBALS.people.get_active_person_write_relays();
                        for relay_url in relays {
                            profile.relays.push(UncheckedUrl(format!("{}", relay_url)));
                        }
                        o.copied_text = format!("https://njump.me/{}", profile.as_bech32_string())
                    });
                }
            });

            if person.petname.is_some() {
                ui.label(RichText::new("†").color(app.theme.accent_complementary_color()))
                    .on_hover_text("trusted petname");
            }

            if followed {
                ui.label(RichText::new("🚶").small())
                    .on_hover_text("followed");
            }

            if !profile_page {
                if let Some(mut nip05) = person.nip05().map(|s| s.to_owned()) {
                    if nip05.starts_with("_@") {
                        nip05 = nip05.get(2..).unwrap().to_string();
                    }

                    ui.with_layout(
                        Layout::left_to_right(Align::Min)
                            .with_cross_align(Align::Center)
                            .with_cross_justify(true),
                        |ui| {
                            if person.nip05_valid {
                                ui.label(RichText::new(nip05).monospace().small());
                            } else {
                                ui.label(RichText::new(nip05).monospace().small().strikethrough());
                            }
                        },
                    );
                }
            }
        });
    }

    pub fn try_get_avatar(&mut self, ctx: &Context, pubkey: &PublicKey) -> Option<TextureHandle> {
        // Do not keep retrying if failed
        if GLOBALS.failed_avatars.read().contains(pubkey) {
            return None;
        }

        if let Some(th) = self.avatars.get(pubkey) {
            return Some(th.to_owned());
        }

        if let Some(rgba_image) =
            GLOBALS
                .people
                .get_avatar(pubkey, self.theme.round_image(), crate::AVATAR_SIZE)
        {
            let current_size = [rgba_image.width() as usize, rgba_image.height() as usize];
            let pixels = rgba_image.as_flat_samples();
            let color_image = ColorImage::from_rgba_unmultiplied(current_size, pixels.as_slice());
            let texture_handle = ctx.load_texture(
                pubkey.as_hex_string(),
                color_image,
                TextureOptions::default(),
            );
            self.avatars
                .insert(pubkey.to_owned(), texture_handle.clone());
            Some(texture_handle)
        } else {
            None
        }
    }

    pub fn try_check_url(&self, url_string: &str) -> Option<Url> {
        let unchecked_url = UncheckedUrl(url_string.to_owned());
        GLOBALS.media.check_url(unchecked_url)
    }

    pub fn retry_media(&self, url: &Url) {
        GLOBALS.media.retry_failed(&url.to_unchecked_url());
    }

    pub fn has_media_loading_failed(&self, url_string: &str) -> Option<String> {
        let unchecked_url = UncheckedUrl(url_string.to_owned());
        GLOBALS.media.has_failed(&unchecked_url)
    }

    pub fn try_get_media(
        &mut self,
        ctx: &Context,
        url: Url,
        volatile: bool,
        file_metadata: Option<&FileMetadata>,
    ) -> MediaLoadingResult<TextureHandle> {
        // Do not keep retrying if failed
        if let Some(failure) = GLOBALS.media.has_failed(&url.to_unchecked_url()) {
            return MediaLoadingResult::Failed(failure);
        }

        // see if we already have a texturehandle for this media
        if let Some(th) = self.images.get(&url) {
            return MediaLoadingResult::Ready(th.to_owned());
        }

        match GLOBALS.media.get_image(&url, volatile, file_metadata) {
            MediaLoadingResult::Disabled => MediaLoadingResult::Disabled,
            MediaLoadingResult::Loading => {
                if let Some(fm) = file_metadata {
                    let (w, h) = match fm.dim {
                        Some((w, h)) => (w, h),
                        None => (400, 400),
                    };
                    if let Some(bh) = &fm.blurhash {
                        if let Some(texture_handle) = self.blurs.get(&url) {
                            return MediaLoadingResult::Ready(texture_handle.clone());
                        } else {
                            if let Ok(rgba_image) =
                                blurhash::decode_image(bh, w as u32, h as u32, 1.0)
                            {
                                let current_size =
                                    [rgba_image.width() as usize, rgba_image.height() as usize];
                                let pixels = rgba_image.as_flat_samples();
                                let color_image = ColorImage::from_rgba_unmultiplied(
                                    current_size,
                                    pixels.as_slice(),
                                );
                                let texture_handle = ctx.load_texture(
                                    url.as_str().to_owned(),
                                    color_image,
                                    TextureOptions::default(),
                                );
                                self.blurs.insert(url, texture_handle.clone());
                                return MediaLoadingResult::Ready(texture_handle);
                            }
                        }
                    }
                }
                MediaLoadingResult::Loading
            }
            MediaLoadingResult::Ready(rgba_image) => {
                let current_size = [rgba_image.width() as usize, rgba_image.height() as usize];
                let pixels = rgba_image.as_flat_samples();
                let color_image =
                    ColorImage::from_rgba_unmultiplied(current_size, pixels.as_slice());
                let texture_handle = ctx.load_texture(
                    url.as_str().to_owned(),
                    color_image,
                    TextureOptions::default(),
                );

                self.images.insert(url, texture_handle.clone());
                MediaLoadingResult::Ready(texture_handle)
            }
            MediaLoadingResult::Failed(s) => MediaLoadingResult::Failed(s),
        }
    }

    #[cfg(feature = "video-ffmpeg")]
    pub fn try_get_player(
        &mut self,
        ctx: &Context,
        url: Url,
        volatile: bool,
        file_metadata: Option<&FileMetadata>,
    ) -> MediaLoadingResult<Rc<RefCell<egui_video::Player>>> {
        // Do not keep retrying if failed
        if let Some(failure) = GLOBALS.media.has_failed(&url.to_unchecked_url()) {
            return MediaLoadingResult::Failed(failure);
        }

        // see if we already have a player for this video
        if let Some(player) = self.video_players.get(&url) {
            return MediaLoadingResult::Ready(player.to_owned());
        }

        match GLOBALS.media.get_data(&url, volatile, file_metadata) {
            MediaLoadingResult::Disabled => MediaLoadingResult::Disabled,
            MediaLoadingResult::Loading => MediaLoadingResult::Loading,
            MediaLoadingResult::Ready(bytes) => {
                if let Ok(player) = Player::new_from_bytes(ctx, &bytes) {
                    if let Some(audio) = &mut self.audio_device {
                        if let Ok(player) = player.with_audio(audio) {
                            let player_ref = Rc::new(RefCell::new(player));
                            self.video_players.insert(url.clone(), player_ref.clone());
                            MediaLoadingResult::Ready(player_ref)
                        } else {
                            let failure = "Player setup with audio failed".to_owned();
                            GLOBALS
                                .media
                                .set_has_failed(&url.to_unchecked_url(), failure.clone());
                            MediaLoadingResult::Failed(failure)
                        }
                    } else {
                        let player_ref = Rc::new(RefCell::new(player));
                        self.video_players.insert(url.clone(), player_ref.clone());
                        MediaLoadingResult::Ready(player_ref)
                    }
                } else {
                    let failure = "Player setup failed".to_owned();
                    GLOBALS
                        .media
                        .set_has_failed(&url.to_unchecked_url(), failure.clone());
                    MediaLoadingResult::Failed(failure)
                }
            }
            MediaLoadingResult::Failed(s) => MediaLoadingResult::Failed(s),
        }
    }

    pub fn show_qr(&mut self, ui: &mut Ui, key: &str) {
        match self.qr_codes.get(key) {
            Some(Ok((texture_handle, x, y))) => {
                ui.add(
                    Image::new(texture_handle)
                        .max_size(Vec2 { x: *x, y: *y })
                        .maintain_aspect_ratio(true),
                );
            }
            Some(Err(error)) => {
                ui.label(
                    RichText::new(format!("CANNOT LOAD QR: {}", error))
                        .color(Color32::from_rgb(160, 0, 0)),
                );
            }
            None => {}
        }
    }

    pub fn render_qr(&mut self, ui: &mut Ui, key: &str, content: &str) {
        self.generate_qr(ui, key, content);
        self.show_qr(ui, key);
    }

    pub fn generate_qr(
        &mut self,
        ui: &mut Ui,
        key: &str,
        content: &str,
    ) -> Option<(TextureHandle, f32, f32)> {
        // Remember the UI runs this every frame.  We do NOT want to load the texture to the GPU
        // every frame, so we remember the texture handle in app.qr_codes, and only load to the GPU
        // if we don't have it yet.  We also remember if there was an error and don't try again.
        match self.qr_codes.get(key) {
            Some(result) => match result {
                Ok((th, x, y)) => Some((th.clone(), *x, *y)),
                Err(_) => None,
            },
            None => {
                // need bytes
                if let Ok(code) = qrcode::QrCode::new(content) {
                    let image = code.render::<image::Rgba<u8>>().build();

                    let color_image = ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_flat_samples().as_slice(),
                    );

                    let texture_handle =
                        ui.ctx()
                            .load_texture(key, color_image, TextureOptions::default());

                    // Convert image size into points for later rendering (so that it renders with
                    // the number of pixels recommended by the qrcode library)
                    let ppp = ui.ctx().pixels_per_point();
                    let x = image.width() as f32 / ppp;
                    let y = image.height() as f32 / ppp;

                    self.qr_codes
                        .insert(key.to_string(), Ok((texture_handle.clone(), x, y)));

                    Some((texture_handle, x, y))
                } else {
                    self.qr_codes.insert(
                        key.to_string(),
                        Err(("Could not make a QR", file!(), line!()).into()),
                    );

                    None
                }
            }
        }
    }

    pub fn delete_qr(&mut self, key: &str) {
        self.qr_codes.remove(key);
    }

    fn add_menu_item_page(
        &mut self,
        ui: &mut Ui,
        page: Page,
        title: Option<&str>,
        highlight: bool,
    ) {
        let condition = if highlight { self.page == page } else { false };

        let pagename;

        let title = match title {
            Some(t) => t,
            None => {
                pagename = page.name();
                &pagename
            }
        };

        if self.add_selected_label(ui, condition, title).clicked() {
            self.set_page(ui.ctx(), page);
        }
    }

    fn open_menu(&mut self, ctx: &Context, item: SubMenu) {
        for (submenu, id) in self.submenu_ids.iter() {
            let mut cstate =
                egui::collapsing_header::CollapsingState::load_with_default_open(ctx, *id, false);
            if item == SubMenu::Feeds || *submenu != SubMenu::Feeds {
                cstate.set_open(*submenu == item);
            }
            cstate.store(ctx);
        }
    }

    fn close_all_menus_except_feeds(&mut self, ctx: &Context) {
        for (submenu, id) in self.submenu_ids.iter() {
            let mut cstate =
                egui::collapsing_header::CollapsingState::load_with_default_open(ctx, *id, false);
            if *submenu != SubMenu::Feeds {
                cstate.set_open(false);
            }
            cstate.store(ctx);
        }
    }

    fn get_openable_menu(
        &mut self,
        ui: &mut Ui,
        ctx: &Context,
        submenu: SubMenu,
    ) -> (egui::collapsing_header::CollapsingState, Response) {
        let mut cstate = egui::collapsing_header::CollapsingState::load_with_default_open(
            ctx,
            self.submenu_ids[&submenu],
            false,
        );
        let open = cstate.is_open();
        let txt = if open {
            submenu.to_string() + " \u{25BE}"
        } else {
            submenu.to_string() + " \u{25B8}"
        };
        if open {
            ui.add_space(10.0)
        }
        let header_res = ui.horizontal(|ui| {
            if ui.add(self.new_header_label(open, txt.as_str())).clicked() {
                if open {
                    cstate.set_open(false);
                    cstate.store(ctx);
                } else {
                    self.open_menu(ctx, submenu);
                    // Local cstate variable does not get updated by the above
                    // call so we have to update it here, but do not have to
                    // store it.
                    cstate.set_open(true);
                }
            }
        });
        header_res
            .response
            .clone()
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        (cstate, header_res.response)
    }

    fn after_openable_menu(&self, ui: &mut Ui, cstate: &egui::collapsing_header::CollapsingState) {
        if cstate.is_open() {
            ui.add_space(10.0)
        }
    }

    fn new_header_label(&self, is_open: bool, text: &str) -> NavItem {
        NavItem::new(text, is_open)
            .color(self.theme.navigation_text_color())
            .active_color(self.theme.navigation_header_active_color())
            .hover_color(self.theme.navigation_text_hover_color())
            .sense(Sense::click())
    }

    fn new_selected_label(&self, selected: bool, text: &str) -> NavItem {
        NavItem::new(text, selected)
            .color(self.theme.navigation_text_color())
            .active_color(self.theme.navigation_text_active_color())
            .hover_color(self.theme.navigation_text_hover_color())
            .sense(Sense::click())
    }

    fn add_selected_label(&self, ui: &mut Ui, selected: bool, text: &str) -> egui::Response {
        let label = self.new_selected_label(selected, text);
        ui.add_space(2.0);
        let response = ui.add(label);
        ui.add_space(2.0);
        response
    }

    // This function is to help change our subscriptions to augmenting events as we scroll.
    fn handle_visible_note_changes(&mut self) {
        let no_change = self.visible_note_ids == self.next_visible_note_ids;
        let scrolling = self.current_scroll_offset != 0.0;
        let too_rapid = Instant::now() - self.last_visible_update < Duration::from_secs(2);

        if no_change || scrolling || too_rapid {
            // Clear the accumulator
            // It will fill up again next frame and be tested again.
            self.next_visible_note_ids.clear();
            return;
        }

        // Update when this happened, so we don't accept again too rapidly
        self.last_visible_update = Instant::now();

        // Save to self.visible_note_ids
        self.visible_note_ids = std::mem::take(&mut self.next_visible_note_ids);

        if !self.visible_note_ids.is_empty() {
            tracing::trace!(
                "VISIBLE = {:?}",
                self.visible_note_ids
                    .iter()
                    .map(|id| id.as_hex_string().as_str().get(0..10).unwrap().to_owned())
                    .collect::<Vec<_>>()
            );

            // Tell the overlord
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::VisibleNotesChanged(
                    self.visible_note_ids.clone(),
                ));
        }
    }

    // Zap In Progress Area
    fn render_zap_area(&mut self, ui: &mut Ui) {
        let mut qr_string: Option<String> = None;

        match self.zap_state {
            ZapState::None => return, // should not occur
            ZapState::CheckingLnurl(_id, _pubkey, ref _lnurl) => {
                ui.label("Loading lnurl...");
            }
            ZapState::SeekingAmount(id, pubkey, ref _prd, ref _lnurl) => {
                ui.vertical(|ui| {
                    let mut amt = 0;

                    ui.horizontal(|ui| {
                        ui.label("Zap Amount:");

                        let amounts = [1, 2, 5, 10, 21, 46, 100, 215, 464, 1000, 2154, 4642, 10000];
                        for &amount in &amounts {
                            if ui.button(amount.to_string()).clicked() {
                                amt = amount;
                            }
                        }

                        if ui.button("Cancel").clicked() {
                            *GLOBALS.current_zap.write() = ZapState::None;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.add(
                            Slider::new(&mut self.zap_amount_input, 1..=1000000).text("Zap Amount"),
                        );
                        if ui.button("Zap!").clicked() {
                            amt = self.zap_amount_input;
                        }
                    });

                    if amt > 0 {
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::Zap(
                            id,
                            pubkey,
                            MilliSatoshi(amt * 1_000),
                            "".to_owned(),
                        ));
                    }
                });
            }
            ZapState::LoadingInvoice(_id, _pubkey) => {
                ui.label("Loading zap invoice...");
            }
            ZapState::ReadyToPay(_id, ref invoice) => {
                // we have to copy it and get out of the borrow first
                qr_string = Some(invoice.to_owned());
            }
        };

        if let Some(qr) = qr_string {
            // Show the QR code and a close button
            self.render_qr(ui, "zap", &qr.to_uppercase());
            if ui.button("Close").clicked() {
                *GLOBALS.current_zap.write() = ZapState::None;
            }
        }
    }

    fn reset_draft(&mut self) {
        if let Page::Feed(FeedKind::DmChat(_)) = &self.page {
            self.dm_draft_data.clear();
            self.dm_draft_data_target = None;
        } else {
            self.previous_draft_data = self.draft_data.clone();
            self.draft_data.clear();
            self.show_post_area = false;
            self.draft_needs_focus = false;
        }
    }

    fn show_post_area_fn(&self) -> bool {
        if self.page == Page::DmChatList {
            return false;
        }

        self.show_post_area || matches!(self.page, Page::Feed(FeedKind::DmChat(_)))
    }

    #[inline]
    fn vert_scroll_area(&self) -> ScrollArea {
        ScrollArea::vertical().enable_scrolling(self.enable_ui())
    }

    #[inline]
    fn render_spinner(&self, ui: &mut Ui) {
        let spinner = match self.frame_count % 8 {
            0 => "⣾",
            1 => "⣽",
            2 => "⣻",
            3 => "⢿",
            4 => "⡿",
            5 => "⣟",
            6 => "⣯",
            7 => "⣷",
            /*
            0 => "┤",
            1 => "┘",
            2 => "┴",
            3 => "└",
            4 => "├",
            5 => "┌",
            6 => "┬",
            7 => "┐",
            */
            _ => " ",
        };
        ui.label(spinner);
    }

    fn render_status_queue_area(&self, ui: &mut Ui) {
        let messages = GLOBALS.status_queue.read().read_all();
        if ui
            .add(Label::new(RichText::new(&messages[0])).sense(Sense::click()))
            .clicked()
        {
            GLOBALS.status_queue.write().dismiss(0);
        }
        if ui
            .add(Label::new(RichText::new(&messages[1]).small()).sense(Sense::click()))
            .clicked()
        {
            GLOBALS.status_queue.write().dismiss(1);
        }
        if ui
            .add(Label::new(RichText::new(&messages[2]).weak().small()).sense(Sense::click()))
            .clicked()
        {
            GLOBALS.status_queue.write().dismiss(2);
        }
    }
}

impl eframe::App for GossipUi {
    // This runs whenever repainting has been scheduled. Egui detects and schedules things
    // like mouse input, cursor flash, fade in, etc, and schedules repaints for these.
    //
    // We also have to schedule repaints when something changes with things like
    // ctx.schedule_repaint()
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        // Run only on first frame
        if self.frame_count == 0 {
            // Initialize scaling, now that we have a Viewport
            self.init_scaling(ctx);

            // Set initial menu state, Feed open since initial page is Following.
            self.open_menu(ctx, SubMenu::Feeds);

            // Init first page
            self.set_page_inner(ctx, self.page.clone());
        }

        self.frame_count += 1;

        // Enforce FPS limiting.
        // No amount of notifies or request_repaint()s can bypass this.
        let presleep_now = Instant::now();
        if self.next_frame > presleep_now {
            std::thread::sleep(self.next_frame - presleep_now);
        }
        self.next_frame = {
            let max_fps = read_setting!(max_fps) as f32;
            Instant::now() + Duration::from_secs_f32(1.0 / max_fps)
        };

        // tracing::warn!("REPAINT: {:?}", ctx.repaint_causes());

        if *GLOBALS.read_runstate.borrow() == RunState::ShuttingDown {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // How much scrolling has been requested by inputs during this frame?
        let compose_area_is_focused =
            ctx.memory(|mem| mem.has_focus(egui::Id::new("compose_area")));
        let mut requested_scroll: f32 = 0.0;
        ctx.input(|i| {
            // Consider mouse inputs
            requested_scroll = i.raw_scroll_delta.y * read_setting!(mouse_acceleration);

            // Consider keyboard inputs unless compose area is focused
            if !compose_area_is_focused {
                if i.key_pressed(egui::Key::ArrowDown) {
                    requested_scroll -= 50.0;
                }
                if i.key_pressed(egui::Key::ArrowUp) {
                    requested_scroll += 50.0;
                }
                if i.key_pressed(egui::Key::PageUp) {
                    let screen_rect = i.screen_rect;
                    let window_height = screen_rect.max.y - screen_rect.min.y;
                    requested_scroll += window_height * 0.75;
                }
                if i.key_pressed(egui::Key::PageDown) {
                    let screen_rect = i.screen_rect;
                    let window_height = screen_rect.max.y - screen_rect.min.y;
                    requested_scroll -= window_height * 0.75;
                }
            }
        });

        // Inertial scrolling
        if read_setting!(inertial_scrolling) {
            // Apply some of the requested scrolling, and save some for later so that
            // scrolling is animated and not instantaneous.
            {
                self.future_scroll_offset += requested_scroll;

                let now = Instant::now();
                let duration = if self.next_frame > now {
                    self.next_frame - Instant::now()
                } else {
                    Duration::from_secs_f32(0.0)
                };

                self.current_scroll_offset =
                    self.future_scroll_offset * 5.0 * duration.as_secs_f32();

                self.future_scroll_offset -= self.current_scroll_offset;

                // Friction stop when slow enough
                if self.future_scroll_offset < 1.0 && self.future_scroll_offset > -1.0 {
                    self.future_scroll_offset = 0.0;
                }
            }

            if self.future_scroll_offset != 0.0 {
                // This is probably already handled by egui/src/response.rs:656
                ctx.request_repaint(); // repaint immediately for smooth scrolling
            }
        } else {
            // Changes to the input state have no effect on the scrolling, because it was copied
            // into a private FrameState at the start of the frame.
            // So we have to use current_scroll_offset to do this
            self.current_scroll_offset = requested_scroll;
        }

        ctx.input_mut(|i| {
            i.smooth_scroll_delta.y = self.current_scroll_offset;
        });

        // F11 maximizes
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            let maximized = matches!(ctx.input(|i| i.viewport().maximized), Some(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }

        let mut reapply = false;
        let mut theme = Theme::from_settings();
        if read_setting!(follow_os_dark_mode) {
            // detect if the OS has changed dark/light mode
            let os_dark_mode = ctx.style().visuals.dark_mode;
            if os_dark_mode != theme.dark_mode {
                // switch to the OS setting
                write_setting!(dark_mode, os_dark_mode);
                theme.dark_mode = os_dark_mode;
                reapply = true;
            }
        }
        if self.theme != theme {
            self.theme = theme;
            reapply = true;
        }
        if reapply {
            theme::apply_theme(&self.theme, ctx);
        }

        notifications::calc(self);

        // dialogues first
        if relays::is_entry_dialog_active(self) {
            relays::entry_dialog(ctx, self);
        }

        // If login is forced, it takes over
        if GLOBALS.wait_for_login.load(Ordering::Relaxed) {
            return force_login(self, ctx);
        }

        // If data migration, show that screen
        if GLOBALS.wait_for_data_migration.load(Ordering::Relaxed) {
            return wait_for_data_migration(self, ctx);
        }

        // If database is being pruned, show that screen
        let optstatus = GLOBALS.prune_status.read();
        if let Some(status) = optstatus.as_ref() {
            return wait_for_prune(self, ctx, status);
        }

        // Wizard does its own panels
        if let Page::Wizard(wp) = self.page {
            return wizard::update(self, ctx, frame, wp);
        }

        // Modal dialogue
        if let Some(entry) = &self.modal {
            if widgets::modal_popup_dyn(ctx, self, true, entry.clone())
                .inner
                .clicked()
            {
                self.modal = None;
            }
        }

        // Bottom Panel
        self.bottom_panel(ctx);

        // Side panel
        self.side_panel(ctx);

        let (show_top_post_area, show_bottom_post_area) = if self.show_post_area_fn() {
            let posting_area_at_top = if matches!(self.page, Page::Feed(FeedKind::DmChat(_))) {
                read_setting!(dm_posting_area_at_top)
            } else {
                read_setting!(posting_area_at_top)
            };

            if posting_area_at_top {
                (true, false)
            } else {
                (false, true)
            }
        } else {
            (false, false)
        };

        let has_warning = {
            #[cfg(feature = "video-ffmpeg")]
            {
                !self.warn_no_libsdl2_dismissed && self.audio_device.is_none()
            }
            #[cfg(not(feature = "video-ffmpeg"))]
            {
                false
            }
        };

        egui::TopBottomPanel::top("top-panel")
            .frame(
                egui::Frame::side_top_panel(&self.theme.get_style()).inner_margin(egui::Margin {
                    left: 20.0,
                    right: 15.0,
                    top: 10.0,
                    bottom: 10.0,
                }),
            )
            .resizable(true)
            .show_animated(
                ctx,
                show_top_post_area || has_warning,
                |ui| {
                    self.begin_ui(ui);
                    #[cfg(feature = "video-ffmpeg")]
                    {
                        if has_warning {
                            widgets::warning_frame(ui, self, |ui, app| {
                                ui.label("You have compiled gossip with 'video-ffmpeg' option but no audio device was found on your system. Make sure you have followed the instructions at ");
                                ui.hyperlink("https://github.com/Rust-SDL2/rust-sdl2");
                                ui.label("and installed 'libsdl2-dev' package for your system.");
                                ui.end_row();
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                                    if ui.link("Dismiss message").clicked() {
                                        app.warn_no_libsdl2_dismissed = true;
                                    }
                                });
                            });
                        }
                    }
                    if show_top_post_area {
                        feed::post::posting_area(self, ctx, frame, ui);
                    }
                },
            );

        let resizable = true;

        egui::TopBottomPanel::bottom("bottom-panel")
            .frame({
                let frame = egui::Frame::side_top_panel(&self.theme.get_style());
                frame.inner_margin(egui::Margin {
                    left: 20.0,
                    right: 18.0,
                    top: 10.0,
                    bottom: 10.0,
                })
            })
            .resizable(resizable)
            .show_separator_line(false)
            .show_animated(ctx, show_bottom_post_area, |ui| {
                self.begin_ui(ui);
                if show_bottom_post_area {
                    ui.add_space(7.0);
                    feed::post::posting_area(self, ctx, frame, ui);
                }
            });

        // Prepare local zap data once per frame for easier compute at render time
        self.zap_state = (*GLOBALS.current_zap.read()).clone();
        self.note_being_zapped = match self.zap_state {
            ZapState::None => None,
            ZapState::CheckingLnurl(id, _, _) => Some(id),
            ZapState::SeekingAmount(id, _, _, _) => Some(id),
            ZapState::LoadingInvoice(id, _) => Some(id),
            ZapState::ReadyToPay(id, _) => Some(id),
        };

        egui::CentralPanel::default()
            .frame({
                let frame = egui::Frame::central_panel(&self.theme.get_style());
                frame.inner_margin(egui::Margin {
                    left: 20.0,
                    right: 10.0,
                    top: 10.0,
                    bottom: 0.0,
                })
            })
            .show(ctx, |ui| {
                self.begin_ui(ui);
                match self.page {
                    Page::DmChatList => dm_chat_list::update(self, ctx, frame, ui),
                    Page::Feed(_) => feed::update(self, ctx, ui),
                    Page::HandlerKinds => handler::update_all_kinds(self, ctx, ui),
                    Page::Handlers(kind) => handler::update_kind(self, ctx, ui, kind),
                    Page::Notifications => notifications::update(self, ui),
                    Page::PeopleLists
                    | Page::PeopleList(_)
                    | Page::Person(_)
                    | Page::PersonFollows(_)
                    | Page::PersonFollowers(_) => people::update(self, ctx, frame, ui),
                    Page::YourKeys
                    | Page::YourMetadata
                    | Page::YourDelegation
                    | Page::YourNostrConnect => you::update(self, ctx, frame, ui),
                    Page::RelaysActivityMonitor
                    | Page::RelaysCoverage
                    | Page::RelaysMine
                    | Page::RelaysKnownNetwork(_) => relays::update(self, ctx, frame, ui),
                    Page::SearchLocal => search::update(self, ctx, frame, ui, true),
                    Page::SearchRelays => search::update(self, ctx, frame, ui, false),
                    Page::Settings => settings::update(self, ctx, frame, ui),
                    Page::HelpHelp | Page::HelpStats | Page::HelpAbout => {
                        help::update(self, ctx, frame, ui)
                    }
                    Page::ThemeTest => theme::test_page::update(self, ctx, frame, ui),
                    Page::Wizard(_) => unreachable!(),
                }
            });
    }
}

fn force_login(app: &mut GossipUi, ctx: &Context) {
    egui::CentralPanel::default()
        .frame({
            let frame = egui::Frame::central_panel(&app.theme.get_style());
            frame.inner_margin(egui::Margin {
                left: 20.0,
                right: 10.0,
                top: 10.0,
                bottom: 0.0,
            })
            .fill({
                if ctx.style().visuals.dark_mode {
                    egui::Color32::from_rgb(0x28, 0x28, 0x28)
                } else {
                    Color32::WHITE
                }
            })
        })
        .show(ctx, |ui| {
            let frame = egui::Frame::none();
            let area = egui::Area::new(ui.auto_id_with("login_screen"))
                .movable(false)
                .interactable(true)
                .constrain(true)
                .order(egui::Order::Middle)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, -100.0]);
            area.show(ui.ctx(), |ui| {
                // frame.rounding = egui::Rounding::same(10.0);
                // frame.inner_margin = egui::Margin::symmetric(MARGIN_X, MARGIN_Y);
                frame.show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(115.0);

                        ui.label(RichText::new("Welcome to Gossip").size(21.0));
                        ui.add_space(8.0);

                        ui.label("Enter your passphrase");
                        // .on_hover_text("In order to AUTH to relays, show DMs, post, zap and react, gossip needs your private key.");

                        ui.label("to unlock the Nostr private key and login");
                        ui.add_space(16.0);

                        let last_status = GLOBALS.status_queue.read().read_last();
                        if !last_status.starts_with("Welcome") {
                            ui.label(RichText::new(last_status).color(app.theme.warning_marker_text_color()));
                            app.unlock_needs_focus = true;
                            ui.add_space(16.0);
                        }

                        let output = widgets::TextEdit::singleline(&app.theme,&mut app.password)
                            .password(true)
                            .with_paste()
                            .desired_width( 400.0)
                            .show(ui);
                        if app.unlock_needs_focus {
                            output.response.request_focus();
                            app.unlock_needs_focus = false;
                        }

                        ui.add_space(20.0);
                        if ui.checkbox(&mut app.unsaved_settings.offline, " Start in offline mode").changed() {
                            let _ = app.unsaved_settings.save();
                        }
                        ui.add_space(20.0);

                        let mut submitted =
                            //response.lost_focus() &&
                            ui.input(|i| i.key_pressed(egui::Key::Enter));

                        ui.scope(|ui| {
                            submitted |= widgets::Button::primary(&app.theme, "     Continue     ").show(ui).clicked();
                        });

                        if submitted {
                            let _ = gossip_lib::Overlord::unlock_key(app.password.clone());
                            app.password.zeroize();
                            app.password = "".to_owned();
                            app.draft_needs_focus = true;
                            // don't cancel login, they may have entered a bad password
                        }

                        ui.add_space(45.0);

                        let data_migration = GLOBALS.wait_for_data_migration.load(Ordering::Relaxed);

                        // If there is a data migration, explain
                        if data_migration {
                            ui.label(RichText::new("Access with public key is not available for this session, a data migration is needed").weak())
                                .on_hover_text("We need to rebuild some data which may require decrypting DMs and Giftwraps to rebuild properly. For this reason, you need to login before the data migration runs.");
                            ui.add_space(30.0);

                            ui.label("In case you cannot login, here is your escape hatch:");
                            if app.delete_confirm {
                                ui.label("Please confirm that you really mean to do this: ");
                                if widgets::Button::primary(&app.theme, "Delete Identity (Yes I'm Sure)")
                                    .with_danger_hover()
                                    .show(ui)
                                    .clicked() {
                                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::DeletePriv);
                                    app.delete_confirm = false;
                                    cancel_login();
                                }
                            } else {
                                if widgets::Button::primary(&app.theme, "Delete Identity (Cannot be undone!)")
                                    .with_danger_hover()
                                    .show(ui)
                                    .clicked() {
                                    app.delete_confirm = true;
                                }
                            }
                        } else {
                            // Change link color:
                            ui.style_mut().visuals.hyperlink_color = ui.style_mut().visuals.widgets.noninteractive.fg_stroke.color;

                            if ui.link("Skip login, browse with public key >>")
                                .on_hover_text("You may skip this if you only want to view public posts, and you can unlock it at a later time under the Account menu.")
                                .clicked() {
                                cancel_login();
                            }
                        }
                    });

                });
            });

            let mut frame = egui::Frame::none();
            let area = egui::Area::new(ui.auto_id_with("login_footer"))
                .movable(false)
                .interactable(true)
                .constrain(true)
                .order(egui::Order::Middle)
                .anchor(egui::Align2::CENTER_BOTTOM, [0.0, 0.0]);
            area.show(ctx, |ui| {
                frame.inner_margin = egui::Margin::symmetric(10.0,40.0);
                frame.show(ui, |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::BOTTOM).with_main_justify(true), |ui| {
                        ui.horizontal( |ui| {
                            // Change link color:
                            ui.style_mut().visuals.hyperlink_color = app.theme.navigation_text_color();

                            ui.label(
                                RichText::new("Do you need help? Open an").weak()
                            );
                            ui.hyperlink_to(
                                "issue on Github",
                                "https://github.com/mikedilger/gossip/issues"
                            );
                            ui.label(
                                RichText::new("or join our").weak()
                            );
                            ui.hyperlink_to(
                                "chat",
                                "https://chachi.chat/groups.0xchat.com/R2yYwhsTcKO2b65i"
                            );
                        });
                    });
                });
            });
        });
}

fn cancel_login() {
    // Stop waiting for login
    GLOBALS
        .wait_for_login
        .store(false, std::sync::atomic::Ordering::Relaxed);
    GLOBALS.wait_for_login_notify.notify_one();
}

fn wait_for_data_migration(app: &mut GossipUi, ctx: &Context) {
    egui::CentralPanel::default()
        .frame({
            let frame = egui::Frame::central_panel(&app.theme.get_style());
            frame.inner_margin(egui::Margin {
                left: 20.0,
                right: 10.0,
                top: 10.0,
                bottom: 0.0,
            })
        })
        .show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                ui.heading("Please wait for the data migration to complete...");
            });
        });
}

fn wait_for_prune(app: &mut GossipUi, ctx: &Context, status: &str) {
    egui::CentralPanel::default()
        .frame({
            let frame = egui::Frame::central_panel(&app.theme.get_style());
            frame.inner_margin(egui::Margin {
                left: 20.0,
                right: 10.0,
                top: 10.0,
                bottom: 0.0,
            })
        })
        .show(ctx, |ui| {
            ui.heading("Please wait for the database prune to complete...");
            ui.label(status);
        });
}

// For the given feed, get the "include all" switch from persisted storage.
fn read_feed_include_all(feed_kind: &FeedKind, ctx: &Context) -> bool {
    let mut key = feed_kind.anchor_key();
    key.push_str("_include_all");
    let egui_id = egui::Id::new(key);
    let include_all: bool = ctx.data_mut(|d| d.get_persisted(egui_id)).unwrap_or(false);
    include_all
}

// For the given feed, set the "include all" switch in persisted storage
fn write_feed_include_all(feed_kind: &FeedKind, ctx: &Context, val: bool) {
    let mut key = feed_kind.anchor_key();
    key.push_str("_include_all");
    let egui_id = egui::Id::new(key);
    ctx.data_mut(|d| d.insert_persisted(egui_id, val));
}
