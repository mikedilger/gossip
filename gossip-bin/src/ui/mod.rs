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
            gossip_lib::GLOBALS.storage.[<read_setting_ $field>]()
        }
    };
}

macro_rules! write_setting {
    ($field:ident, $val:expr) => {
        paste::paste! {
            let _ = gossip_lib::GLOBALS.storage.[<write_setting_ $field>](&$val, None);
        }
    };
}

mod components;
mod dm_chat_list;
mod feed;
mod help;
mod people;
mod relays;
mod search;
mod settings;
mod theme;
mod widgets;
mod wizard;
mod you;

pub use crate::ui::theme::{Theme, ThemeVariant};
use crate::unsaved_settings::UnsavedSettings;
#[cfg(feature = "video-ffmpeg")]
use core::cell::RefCell;
use eframe::{egui, IconData};
use egui::{
    Align, Color32, ColorImage, Context, Image, ImageData, Label, Layout, RichText, ScrollArea,
    Sense, TextureHandle, TextureOptions, Ui, Vec2,
};
#[cfg(feature = "video-ffmpeg")]
use egui_video::{AudioDevice, Player};
use egui_winit::egui::Rect;
use egui_winit::egui::Response;
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::About;
use gossip_lib::Error;
use gossip_lib::FeedKind;
use gossip_lib::{DmChannel, DmChannelData};
use gossip_lib::{Person, PersonList};
use gossip_lib::{ZapState, GLOBALS};
use nostr_types::ContentSegment;
use nostr_types::{Id, Metadata, MilliSatoshi, Profile, PublicKey, UncheckedUrl, Url};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
#[cfg(feature = "video-ffmpeg")]
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use usvg::TreeParsing;
use zeroize::Zeroize;

use self::feed::Notes;
use self::widgets::NavItem;
use self::wizard::{WizardPage, WizardState};

pub fn run() -> Result<(), Error> {
    let icon_bytes = include_bytes!("../../../logo/gossip.png");
    let icon = image::load_from_memory(icon_bytes)?.to_rgba8();
    let (icon_width, icon_height) = icon.dimensions();

    let options = eframe::NativeOptions {
        #[cfg(target_os = "linux")]
        app_id: Some("gossip".to_string()),
        decorated: true,
        #[cfg(target_os = "macos")]
        fullsize_content: true,
        drag_and_drop_support: true,
        default_theme: if read_setting!(dark_mode) {
            eframe::Theme::Dark
        } else {
            eframe::Theme::Light
        },
        icon_data: Some(IconData {
            rgba: icon.into_raw(),
            width: icon_width,
            height: icon_height,
        }),
        initial_window_size: Some(egui::vec2(700.0, 900.0)),
        resizable: true,
        centered: true,
        vsync: true,
        follow_system_theme: read_setting!(follow_os_dark_mode),
        min_window_size: Some(egui::vec2(800.0, 600.0)),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "gossip",
        options,
        Box::new(|cc| Box::new(GossipUi::new(cc))),
    ) {
        tracing::error!("Eframe error: {}", e);
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
enum Page {
    DmChatList,
    Feed(FeedKind),
    PeopleLists,
    PeopleList(PersonList),
    Person(PublicKey),
    YourKeys,
    YourMetadata,
    YourDelegation,
    YourNostrConnect,
    RelaysActivityMonitor,
    RelaysCoverage,
    RelaysMine,
    RelaysKnownNetwork,
    Search,
    Settings,
    HelpHelp,
    HelpStats,
    HelpAbout,
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
            Page::DmChatList => (SubMenu::Feeds.as_str(), "Private chats".into()),
            Page::Feed(feedkind) => ("Feed", feedkind.to_string()),
            Page::PeopleLists => ("Lists", "Lists".into()),
            Page::PeopleList(list) => {
                let metadata = GLOBALS
                    .storage
                    .get_person_list_metadata(*list)
                    .unwrap_or_default()
                    .unwrap_or_default();
                ("Lists", metadata.title)
            }
            Page::Person(pk) => {
                let name = gossip_lib::names::best_name_from_pubkey_lookup(pk);
                ("Profile", name)
            }
            Page::YourKeys => (SubMenu::Account.as_str(), "Keys".into()),
            Page::YourMetadata => (SubMenu::Account.as_str(), "Profile".into()),
            Page::YourDelegation => (SubMenu::Account.as_str(), "Delegation".into()),
            Page::YourNostrConnect => (SubMenu::Account.as_str(), "Nostr Connect".into()),
            Page::RelaysActivityMonitor => (SubMenu::Relays.as_str(), "Active Relays".into()),
            Page::RelaysCoverage => (SubMenu::Relays.as_str(), "Coverage Report".into()),
            Page::RelaysMine => (SubMenu::Relays.as_str(), "My Relays".into()),
            Page::RelaysKnownNetwork => (SubMenu::Relays.as_str(), "Known Network".into()),
            Page::Search => ("Search", "Search".into()),
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
    Relays,
    Account,
    Help,
}

impl SubMenu {
    fn as_str(&self) -> &'static str {
        match self {
            SubMenu::Feeds => "Feeds",
            SubMenu::Relays => "Relays",
            SubMenu::Account => "Account",
            SubMenu::Help => "Help",
        }
    }

    fn as_id_str(&self) -> &'static str {
        match self {
            SubMenu::Feeds => "feeds_submenu_id",
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

    // If the user is typing a @tag, this is what they typed
    pub tagging_search_substring: Option<String>,
    pub tagging_search_selected: Option<usize>,
    pub tagging_search_searched: Option<String>,
    pub tagging_search_results: Vec<(String, PublicKey)>,
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

            tagging_search_substring: None,
            tagging_search_selected: None,
            tagging_search_searched: None,
            tagging_search_results: Vec::new(),
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
        self.tagging_search_substring = None;
        self.tagging_search_selected = None;
        self.tagging_search_searched = None;
        self.tagging_search_results.clear();
    }
}

struct GossipUi {
    #[cfg(feature = "video-ffmpeg")]
    audio_device: Option<AudioDevice>,
    #[cfg(feature = "video-ffmpeg")]
    video_players: HashMap<Url, Rc<RefCell<egui_video::Player>>>,

    initializing: bool,

    // Rendering
    next_frame: Instant,
    override_dpi: bool,
    override_dpi_value: u32,
    original_dpi_value: u32,
    current_scroll_offset: f32,
    future_scroll_offset: f32,

    // clipboard
    clipboard: egui_winit::clipboard::Clipboard,

    // Ui timers
    popups: HashMap<egui::Id, HashMap<egui::Id, Box<dyn widgets::InformationPopup>>>,

    // QR codes being rendered (in feed or elsewhere)
    // the f32's are the recommended image size
    qr_codes: HashMap<String, Result<(TextureHandle, f32, f32), Error>>,

    // Processed events caching
    notes: Notes,

    // RelayUi
    relays: relays::RelayUi,

    // people::ListUi
    people_list: people::ListUi,

    // Post rendering
    render_raw: Option<Id>,
    render_qr: Option<Id>,
    approved: HashSet<Id>, // content warning posts
    height: HashMap<Id, f32>,

    // Person page rendering ('npub', 'nprofile', or 'lud06')
    person_qr: Option<&'static str>,
    setting_active_person: bool,

    // Page
    page: Page,
    history: Vec<Page>,
    mainfeed_include_nonroot: bool,
    inbox_include_indirect: bool,
    submenu_ids: HashMap<SubMenu, egui::Id>,
    settings_tab: SettingsTab,

    // General Data
    about: About,
    icon: TextureHandle,
    placeholder_avatar: TextureHandle,
    options_symbol: TextureHandle,
    unsaved_settings: UnsavedSettings,
    theme: Theme,
    avatars: HashMap<PublicKey, TextureHandle>,
    images: HashMap<Url, TextureHandle>,
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
    dm_draft_data: DraftData,

    // User entry: metadata
    editing_metadata: bool,
    metadata: Metadata,

    // User entry: delegatee tag (as JSON string)
    delegatee_tag_str: String,

    // User entry: general
    add_contact: String,
    add_relay: String, // dep
    password: String,
    password2: String,
    password3: String,
    delete_confirm: bool,
    new_metadata_fieldname: String,
    import_priv: String,
    import_pub: String,
    search: String,
    entering_search_page: bool,
    editing_petname: bool,
    petname: String,
    deleting_list: Option<PersonList>,
    creating_list: bool,
    list_name_field_needs_focus: bool,
    new_list_name: String,
    new_list_favorite: bool,
    renaming_list: Option<PersonList>,
    editing_list_error: Option<String>,
    //nostr_connect_string: String,
    nostr_connect_relay1: String,
    nostr_connect_relay2: String,

    // Collapsed threads
    collapsed: Vec<Id>,

    // Fully opened posts
    opened: HashSet<Id>,

    // Visisble Note IDs
    // (we resubscribe to reactions/zaps/deletes when this changes)
    visible_note_ids: Vec<Id>,
    // This one is built up as rendering happens, then compared
    next_visible_note_ids: Vec<Id>,
    last_visible_update: Instant,

    // Zap state, computed once per frame instead of per note
    // zap_state and note_being_zapped are computed from GLOBALS.current_zap and are
    //   not authoratative.
    zap_state: ZapState,
    note_being_zapped: Option<Id>,

    wizard_state: WizardState,

    theme_test: crate::ui::theme::test_page::ThemeTest,

    // Cached DM Channels
    dm_channel_cache: Vec<DmChannelData>,
    dm_channel_next_refresh: Instant,
    dm_channel_error: Option<String>,
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
        let dpi: u32;
        if let Some(override_dpi) = read_setting!(override_dpi) {
            let ppt: f32 = override_dpi as f32 / 72.0;
            cctx.egui_ctx.set_pixels_per_point(ppt);
            dpi = (ppt * 72.0) as u32;
            tracing::info!("DPI (overridden): {}", dpi);
        } else if let Some(ppt) = cctx.integration_info.native_pixels_per_point {
            cctx.egui_ctx.set_pixels_per_point(ppt);
            dpi = (ppt * 72.0) as u32;
            tracing::info!("DPI (native): {}", dpi);
        } else {
            dpi = (cctx.egui_ctx.pixels_per_point() * 72.0) as u32;
            tracing::info!("DPI (fallback): {}", dpi);
        }

        // Set global pixels_per_point_times_100, used for image scaling.
        GLOBALS.pixels_per_point_times_100.store(
            (cctx.egui_ctx.pixels_per_point() * 100.0) as u32,
            Ordering::Relaxed,
        );

        {
            cctx.egui_ctx.tessellation_options_mut(|to| {
                // Less feathering
                to.feathering = true;
                to.feathering_size_in_pixels = 0.667;

                // Sharper text
                to.round_text_to_pixels = true;
            });
        }

        let mut submenu_ids: HashMap<SubMenu, egui::Id> = HashMap::new();
        submenu_ids.insert(SubMenu::Feeds, egui::Id::new(SubMenu::Feeds.as_id_str()));
        submenu_ids.insert(
            SubMenu::Account,
            egui::Id::new(SubMenu::Account.as_id_str()),
        );
        submenu_ids.insert(SubMenu::Relays, egui::Id::new(SubMenu::Relays.as_id_str()));
        submenu_ids.insert(SubMenu::Help, egui::Id::new(SubMenu::Help.as_id_str()));

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

        // how to load an svg
        let options_symbol = {
            let bytes = include_bytes!("../../../assets/option.svg");
            let opt = usvg::Options {
                dpi: dpi as f32,
                ..Default::default()
            };
            let rtree = usvg::Tree::from_data(bytes, &opt).unwrap();
            let [w, h] = [20_u32, 20_u32];
            let mut pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
            let tree = resvg::Tree::from_usvg(&rtree);
            tree.render(Default::default(), &mut pixmap.as_mut());
            let color_image = ColorImage::from_rgba_unmultiplied([w as _, h as _], pixmap.data());
            cctx.egui_ctx
                .load_texture("options_symbol", color_image, TextureOptions::LINEAR)
        };

        let (override_dpi, override_dpi_value): (bool, u32) = match read_setting!(override_dpi) {
            Some(v) => (true, v),
            None => (false, dpi),
        };

        let mut start_page = Page::Feed(FeedKind::List(PersonList::Followed, false));

        // Possibly enter the wizard instead
        let mut wizard_state: WizardState = Default::default();
        let wizard_complete = GLOBALS.storage.get_flag_wizard_complete();
        if !wizard_complete {
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

        GossipUi {
            #[cfg(feature = "video-ffmpeg")]
            audio_device,
            #[cfg(feature = "video-ffmpeg")]
            video_players: HashMap::new(),
            initializing: true,
            next_frame: Instant::now(),
            override_dpi,
            override_dpi_value,
            original_dpi_value: override_dpi_value,
            current_scroll_offset: 0.0,
            future_scroll_offset: 0.0,
            clipboard: egui_winit::clipboard::Clipboard::new(cctx),
            popups: HashMap::new(),
            qr_codes: HashMap::new(),
            notes: Notes::new(),
            relays: relays::RelayUi::new(),
            people_list: people::ListUi::new(),
            render_raw: None,
            render_qr: None,
            approved: HashSet::new(),
            height: HashMap::new(),
            person_qr: None,
            setting_active_person: false,
            page: start_page,
            history: vec![],
            mainfeed_include_nonroot: cctx
                .egui_ctx
                .data_mut(|d| d.get_persisted(egui::Id::new("mainfeed_include_nonroot")))
                .unwrap_or(false),
            inbox_include_indirect: cctx
                .egui_ctx
                .data_mut(|d| d.get_persisted(egui::Id::new("inbox_include_indirect")))
                .unwrap_or(false),
            submenu_ids,
            settings_tab: SettingsTab::Id,
            about: About::new(),
            icon: icon_texture_handle,
            placeholder_avatar: placeholder_avatar_texture_handle,
            options_symbol,
            unsaved_settings: UnsavedSettings::load(),
            theme,
            avatars: HashMap::new(),
            images: HashMap::new(),
            media_show_list: HashSet::new(),
            media_hide_list: HashSet::new(),
            media_full_width_list: HashSet::new(),
            show_post_area: false,
            draft_needs_focus: false,
            unlock_needs_focus: true,
            draft_data: DraftData::default(),
            dm_draft_data: DraftData::default(),
            editing_metadata: false,
            metadata: Metadata::new(),
            delegatee_tag_str: "".to_owned(),
            add_contact: "".to_owned(),
            add_relay: "".to_owned(),
            password: "".to_owned(),
            password2: "".to_owned(),
            password3: "".to_owned(),
            delete_confirm: false,
            new_metadata_fieldname: String::new(),
            import_priv: "".to_owned(),
            import_pub: "".to_owned(),
            search: "".to_owned(),
            entering_search_page: false,
            editing_petname: false,
            petname: "".to_owned(),
            deleting_list: None,
            creating_list: false,
            list_name_field_needs_focus: false,
            new_list_name: "".to_owned(),
            new_list_favorite: false,
            renaming_list: None,
            editing_list_error: None,
            //nostr_connect_string: "".to_owned(),
            nostr_connect_relay1: "".to_owned(),
            nostr_connect_relay2: "".to_owned(),
            collapsed: vec![],
            opened: HashSet::new(),
            visible_note_ids: vec![],
            next_visible_note_ids: vec![],
            last_visible_update: Instant::now(),
            zap_state: ZapState::None,
            note_being_zapped: None,
            wizard_state,
            theme_test: Default::default(),
            dm_channel_cache: vec![],
            dm_channel_next_refresh: Instant::now(),
            dm_channel_error: None,
        }
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
            Page::Feed(FeedKind::DmChat(channel)) => {
                GLOBALS.feed.set_feed_to_dmchat(channel.to_owned());
                self.close_all_menus_except_feeds(ctx);
            }
            Page::Feed(FeedKind::List(list, with_replies)) => {
                GLOBALS.feed.set_feed_to_main(*list, *with_replies);
                self.open_menu(ctx, SubMenu::Feeds);
            }
            Page::Feed(FeedKind::Inbox(indirect)) => {
                GLOBALS.feed.set_feed_to_inbox(*indirect);
                self.close_all_menus_except_feeds(ctx);
            }
            Page::Feed(FeedKind::Thread {
                id,
                referenced_by,
                author,
            }) => {
                GLOBALS
                    .feed
                    .set_feed_to_thread(*id, *referenced_by, *author);
                self.close_all_menus_except_feeds(ctx);
            }
            Page::Feed(FeedKind::Person(pubkey)) => {
                GLOBALS.feed.set_feed_to_person(pubkey.to_owned());
                self.close_all_menus_except_feeds(ctx);
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
            Page::YourKeys | Page::YourMetadata | Page::YourDelegation | Page::YourNostrConnect => {
                self.open_menu(ctx, SubMenu::Account);
            }
            Page::RelaysActivityMonitor
            | Page::RelaysCoverage
            | Page::RelaysMine
            | Page::RelaysKnownNetwork => {
                self.open_menu(ctx, SubMenu::Relays);
            }
            Page::Search => {
                self.entering_search_page = true;
                self.close_all_menus_except_feeds(ctx);
            }
            Page::Settings => {
                self.close_all_menus_except_feeds(ctx);
            }
            Page::HelpHelp | Page::HelpStats | Page::HelpAbout => {
                self.open_menu(ctx, SubMenu::Help);
            }
            _ => {
                self.close_all_menus_except_feeds(ctx);
            }
        }

        // clear some search state
        GLOBALS.events_being_searched_for.write().clear();

        self.page = page;
    }

    fn side_panel(&mut self, ctx: &Context) {
        egui::SidePanel::left("main-naviation-panel")
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

                // cut indentation
                ui.style_mut().spacing.indent = 0.0;
                ui.visuals_mut().widgets.inactive.fg_stroke.color =
                    self.theme.navigation_text_color();
                ui.visuals_mut().widgets.hovered.fg_stroke.color =
                    self.theme.navigation_text_hover_color();
                ui.visuals_mut().widgets.hovered.fg_stroke.width = 1.0;
                ui.visuals_mut().widgets.active.fg_stroke.color =
                    self.theme.navigation_text_active_color();

                ui.add_space(4.0);
                let back_label_text = RichText::new("‹ Back");
                let label = if self.history.is_empty() {
                    Label::new(
                        back_label_text.color(self.theme.navigation_text_deactivated_color()),
                    )
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

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // ---- Feeds SubMenu ----
                {
                    let (mut cstate, header_response) =
                        self.get_openable_menu(ui, ctx, SubMenu::Feeds);
                    cstate.show_body_indented(&header_response, ui, |ui| {
                        let mut all_lists = GLOBALS
                            .storage
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
                                self.add_menu_item_page(
                                    ui,
                                    Page::Feed(FeedKind::List(list, self.mainfeed_include_nonroot)),
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
                    if self
                        .add_selected_label(
                            ui,
                            self.page == Page::Feed(FeedKind::Inbox(self.inbox_include_indirect)),
                            "Inbox",
                        )
                        .clicked()
                    {
                        self.set_page(
                            ctx,
                            Page::Feed(FeedKind::Inbox(self.inbox_include_indirect)),
                        );
                    }
                }

                // Private chats
                if GLOBALS.identity.is_unlocked() {
                    if self
                        .add_selected_label(ui, self.page == Page::DmChatList, "Private chats")
                        .clicked()
                    {
                        self.set_page(ctx, Page::DmChatList);
                    }
                }

                // Search
                if self
                    .add_selected_label(ui, self.page == Page::Search, "Search")
                    .clicked()
                {
                    self.set_page(ctx, Page::Search);
                }

                ui.add_space(10.0);

                // ---- People Lists ----
                {
                    if self
                        .add_selected_label(ui, self.page == Page::PeopleLists, "People Lists")
                        .clicked()
                    {
                        self.set_page(ctx, Page::PeopleLists);
                    }
                }

                // ---- Relays SubMenu ----
                {
                    let (mut cstate, header_response) =
                        self.get_openable_menu(ui, ctx, SubMenu::Relays);
                    cstate.show_body_indented(&header_response, ui, |ui| {
                        self.add_menu_item_page(ui, Page::RelaysActivityMonitor, None, true);
                        self.add_menu_item_page(ui, Page::RelaysMine, None, true);
                        self.add_menu_item_page(ui, Page::RelaysKnownNetwork, None, true);
                        ui.vertical(|ui| {
                            ui.spacing_mut().button_padding *= 2.0;
                            ui.visuals_mut().widgets.inactive.weak_bg_fill =
                                self.theme.accent_color().linear_multiply(0.2);
                            ui.visuals_mut().widgets.inactive.fg_stroke.width = 1.0;
                            ui.visuals_mut().widgets.hovered.weak_bg_fill =
                                self.theme.navigation_text_color();
                            ui.visuals_mut().widgets.hovered.fg_stroke.color =
                                self.theme.accent_color();
                            if ui
                                .button(RichText::new("Add Relay"))
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                relays::start_entry_dialog(self);
                            }
                        });
                    });
                    self.after_openable_menu(ui, &cstate);
                }

                // ---- Account SubMenu ----
                {
                    let (mut cstate, header_response) =
                        self.get_openable_menu(ui, ctx, SubMenu::Account);
                    cstate.show_body_indented(&header_response, ui, |ui| {
                        self.add_menu_item_page(ui, Page::YourMetadata, None, true);
                        self.add_menu_item_page(ui, Page::YourKeys, None, true);
                        self.add_menu_item_page(ui, Page::YourDelegation, None, true);
                        self.add_menu_item_page(ui, Page::YourNostrConnect, None, true);
                    });
                    self.after_openable_menu(ui, &cstate);
                }

                // ---- Settings ----
                if self
                    .add_selected_label(ui, self.page == Page::Settings, "Settings")
                    .clicked()
                {
                    self.set_page(ctx, Page::Settings);
                }

                // ---- Help SubMenu ----
                {
                    let (mut cstate, header_response) =
                        self.get_openable_menu(ui, ctx, SubMenu::Help);
                    cstate.show_body_indented(&header_response, ui, |ui| {
                        self.add_menu_item_page(ui, Page::HelpHelp, None, true);
                        self.add_menu_item_page(ui, Page::HelpStats, None, true);
                        self.add_menu_item_page(ui, Page::HelpAbout, None, true);
                    });
                    self.after_openable_menu(ui, &cstate);
                }

                #[cfg(debug_assertions)]
                if self
                    .add_selected_label(ui, self.page == Page::ThemeTest, "Theme Test")
                    .clicked()
                {
                    self.set_page(ctx, Page::ThemeTest);
                }

                // -- Status Area
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    // -- DEBUG status area
                    if read_setting!(status_bar) {
                        let in_flight = GLOBALS.fetcher.requests_in_flight();
                        let queued = GLOBALS.fetcher.requests_queued();
                        let m = format!("HTTP: {} / {}", in_flight, queued);
                        ui.add(Label::new(
                            RichText::new(m).color(self.theme.notice_marker_text_color()),
                        ));

                        let subs = GLOBALS.open_subscriptions.load(Ordering::Relaxed);
                        let m = format!("RELAY SUBSC {}", subs);
                        ui.add(Label::new(
                            RichText::new(m).color(self.theme.notice_marker_text_color()),
                        ));

                        let relays = GLOBALS.connected_relays.len();
                        let m = format!("RELAYS CONN {}", relays);
                        ui.add(Label::new(
                            RichText::new(m).color(self.theme.notice_marker_text_color()),
                        ));

                        let events = GLOBALS.storage.get_event_len().unwrap_or(0);
                        let m = format!("EVENTS STOR {}", events);
                        ui.add(Label::new(
                            RichText::new(m).color(self.theme.notice_marker_text_color()),
                        ));

                        let processed = GLOBALS.events_processed.load(Ordering::Relaxed);
                        let m = format!("EVENTS RECV {}", processed);
                        ui.add(Label::new(
                            RichText::new(m).color(self.theme.notice_marker_text_color()),
                        ));

                        ui.separator();
                    }

                    self.render_status_queue_area(ui);
                });

                // ---- "plus icon" ----
                if !self.show_post_area_fn() && self.page.show_post_icon() {
                    let bottom_right = ui.ctx().screen_rect().right_bottom();
                    let pos = bottom_right
                        + Vec2::new(-crate::AVATAR_SIZE_F32 * 2.0, -crate::AVATAR_SIZE_F32 * 2.0);

                    egui::Area::new(ui.next_auto_id())
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
                                    let response = ui.add_sized(
                                        [crate::AVATAR_SIZE_F32, crate::AVATAR_SIZE_F32],
                                        egui::Button::new(
                                            text.color(self.theme.get_style().visuals.panel_fill),
                                        )
                                        .stroke(egui::Stroke::NONE)
                                        .rounding(egui::Rounding::same(crate::AVATAR_SIZE_F32))
                                        .fill(self.theme.accent_color()),
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
            });
    }
}

impl eframe::App for GossipUi {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        // Run only on first frame
        if self.initializing {
            self.initializing = false;

            // Set initial menu state, Feed open since initial page is Following.
            self.open_menu(ctx, SubMenu::Feeds);
        }

        let max_fps = read_setting!(max_fps) as f32;

        if self.future_scroll_offset != 0.0 {
            ctx.request_repaint();
        } else {
            // Wait until the next frame
            std::thread::sleep(self.next_frame - Instant::now());
            self.next_frame += Duration::from_secs_f32(1.0 / max_fps);

            // Redraw at least once per second
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        if GLOBALS.shutting_down.load(Ordering::Relaxed) {
            frame.close();
            return;
        }

        // How much scrolling has been requested by inputs during this frame?
        let mut requested_scroll: f32 = 0.0;
        ctx.input(|i| {
            // Consider mouse inputs
            requested_scroll = i.scroll_delta.y * read_setting!(mouse_acceleration);

            // Consider keyboard inputs unless compose area is focused
            if !ctx.memory(|mem| mem.has_focus(egui::Id::new("compose_area"))) {
                if i.key_pressed(egui::Key::ArrowDown) {
                    requested_scroll -= 50.0;
                }
                if i.key_pressed(egui::Key::ArrowUp) {
                    requested_scroll += 50.0;
                }
                if i.key_pressed(egui::Key::PageUp) {
                    let screen_rect = ctx.input(|i| i.screen_rect);
                    let window_height = screen_rect.max.y - screen_rect.min.y;
                    requested_scroll += window_height * 0.75;
                }
                if i.key_pressed(egui::Key::PageDown) {
                    let screen_rect = ctx.input(|i| i.screen_rect);
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

                // Move by 10% of future scroll offsets
                self.current_scroll_offset = 0.1 * self.future_scroll_offset;
                self.future_scroll_offset -= self.current_scroll_offset;

                // Friction stop when slow enough
                if self.future_scroll_offset < 1.0 && self.future_scroll_offset > -1.0 {
                    self.future_scroll_offset = 0.0;
                }
            }
        } else {
            // Changes to the input state have no effect on the scrolling, because it was copied
            // into a private FrameState at the start of the frame.
            // So we have to use current_scroll_offset to do this
            self.current_scroll_offset = requested_scroll;
        }

        let mut reapply = false;
        let mut theme = Theme::from_settings();
        if theme.follow_os_dark_mode {
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

        // Wizard does its own panels
        if let Page::Wizard(wp) = self.page {
            return wizard::update(self, ctx, frame, wp);
        }

        // Side panel
        self.side_panel(ctx);

        egui::TopBottomPanel::top("top-area")
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
                self.show_post_area_fn() && read_setting!(posting_area_at_top),
                |ui| {
                    self.begin_ui(ui);
                    feed::post::posting_area(self, ctx, frame, ui);
                },
            );

        let show_status = self.show_post_area_fn() && !read_setting!(posting_area_at_top);

        let resizable = true;

        egui::TopBottomPanel::bottom("status")
            .frame({
                let frame = egui::Frame::side_top_panel(&self.theme.get_style());
                frame.inner_margin(if !read_setting!(posting_area_at_top) {
                    egui::Margin {
                        left: 20.0,
                        right: 18.0,
                        top: 10.0,
                        bottom: 10.0,
                    }
                } else {
                    egui::Margin {
                        left: 20.0,
                        right: 18.0,
                        top: 1.0,
                        bottom: 5.0,
                    }
                })
            })
            .resizable(resizable)
            .show_separator_line(false)
            .show_animated(ctx, show_status, |ui| {
                self.begin_ui(ui);
                if self.show_post_area_fn() && !read_setting!(posting_area_at_top) {
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
                frame
                    .inner_margin(egui::Margin {
                        left: 20.0,
                        right: 10.0,
                        top: 10.0,
                        bottom: 0.0,
                    })
                    .fill({
                        match self.page {
                            Page::Person(_) => {
                                if self.theme.dark_mode {
                                    ctx.style().visuals.panel_fill
                                } else {
                                    self.theme.main_content_bgcolor()
                                }
                            }
                            _ => ctx.style().visuals.panel_fill,
                        }
                    })
            })
            .show(ctx, |ui| {
                self.begin_ui(ui);
                match self.page {
                    Page::DmChatList => dm_chat_list::update(self, ctx, frame, ui),
                    Page::Feed(_) => feed::update(self, ctx, frame, ui),
                    Page::PeopleLists | Page::PeopleList(_) | Page::Person(_) => {
                        people::update(self, ctx, frame, ui)
                    }
                    Page::YourKeys
                    | Page::YourMetadata
                    | Page::YourDelegation
                    | Page::YourNostrConnect => you::update(self, ctx, frame, ui),
                    Page::RelaysActivityMonitor
                    | Page::RelaysCoverage
                    | Page::RelaysMine
                    | Page::RelaysKnownNetwork => relays::update(self, ctx, frame, ui),
                    Page::Search => search::update(self, ctx, frame, ui),
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

impl GossipUi {
    fn begin_ui(&self, ui: &mut Ui) {
        // if a dialog is open, disable the rest of the UI
        ui.set_enabled(!relays::is_entry_dialog_active(self) && self.person_qr.is_none());
    }

    pub fn person_name(person: &Person) -> String {
        if let Some(petname) = &person.petname {
            petname.clone()
        } else if let Some(display_name) = person.display_name() {
            display_name.to_string()
        } else if let Some(name) = person.name() {
            name.to_string()
        } else if let Some(nip05) = person.nip05() {
            nip05.to_string()
        } else {
            gossip_lib::names::pubkey_short(&person.pubkey)
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
                    match &person.petname {
                        Some(pn) => pn.to_owned(),
                        None => person.best_name(),
                    }
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
                        true,
                        true,
                    );
                } else if followed && ui.button("Unfollow").clicked() {
                    let _ = GLOBALS.people.follow(
                        &person.pubkey,
                        false,
                        PersonList::Followed,
                        true,
                        false,
                    );
                }

                // Do not show 'Mute' if this is yourself
                if muted || !is_self {
                    let mute_label = if muted { "Unmute" } else { "Mute" };
                    if ui.button(mute_label).clicked() {
                        let _ = GLOBALS.people.mute(&person.pubkey, !muted, true);
                        app.notes.cache_invalidate_person(&person.pubkey);
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
                        for (relay_url, _) in relays.iter().take(3) {
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
        if GLOBALS.failed_avatars.blocking_read().contains(pubkey) {
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

    pub fn has_media_loading_failed(&self, url_string: &str) -> bool {
        let unchecked_url = UncheckedUrl(url_string.to_owned());
        GLOBALS.media.has_failed(&unchecked_url)
    }

    pub fn try_get_media(&mut self, ctx: &Context, url: Url) -> Option<TextureHandle> {
        // Do not keep retrying if failed
        if GLOBALS.media.has_failed(&url.to_unchecked_url()) {
            return None;
        }

        // see if we already have a texturehandle for this media
        if let Some(th) = self.images.get(&url) {
            return Some(th.to_owned());
        }

        if let Some(rgba_image) = GLOBALS.media.get_image(&url) {
            let current_size = [rgba_image.width() as usize, rgba_image.height() as usize];
            let pixels = rgba_image.as_flat_samples();
            let color_image = ColorImage::from_rgba_unmultiplied(current_size, pixels.as_slice());
            let texture_handle = ctx.load_texture(
                url.as_str().to_owned(),
                color_image,
                TextureOptions::default(),
            );
            self.images.insert(url, texture_handle.clone());
            Some(texture_handle)
        } else {
            None
        }
    }

    #[cfg(feature = "video-ffmpeg")]
    pub fn try_get_player(
        &mut self,
        ctx: &Context,
        url: Url,
    ) -> Option<Rc<RefCell<egui_video::Player>>> {
        // Do not keep retrying if failed
        if GLOBALS.media.has_failed(&url.to_unchecked_url()) {
            return None;
        }

        // see if we already have a player for this video
        if let Some(player) = self.video_players.get(&url) {
            return Some(player.to_owned());
        }

        if let Some(bytes) = GLOBALS.media.get_data(&url) {
            if let Ok(player) = Player::new_from_bytes(ctx, &bytes) {
                if let Some(audio) = &mut self.audio_device {
                    if let Ok(player) = player.with_audio(audio) {
                        let player_ref = Rc::new(RefCell::new(player));
                        self.video_players.insert(url.clone(), player_ref.clone());
                        Some(player_ref)
                    } else {
                        GLOBALS.media.has_failed(&url.to_unchecked_url());
                        None
                    }
                } else {
                    let player_ref = Rc::new(RefCell::new(player));
                    self.video_players.insert(url.clone(), player_ref.clone());
                    Some(player_ref)
                }
            } else {
                GLOBALS.media.has_failed(&url.to_unchecked_url());
                None
            }
        } else {
            None
        }
    }

    pub fn render_qr(&mut self, ui: &mut Ui, ctx: &Context, key: &str, content: &str) {
        // Remember the UI runs this every frame.  We do NOT want to load the texture to the GPU
        // every frame, so we remember the texture handle in app.qr_codes, and only load to the GPU
        // if we don't have it yet.  We also remember if there was an error and don't try again.
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
            None => {
                // need bytes
                if let Ok(code) = qrcode::QrCode::new(content) {
                    let image = code.render::<image::Rgba<u8>>().build();

                    let color_image = ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_flat_samples().as_slice(),
                    );

                    let texture_handle =
                        ctx.load_texture(key, color_image, TextureOptions::default());

                    // Convert image size into points for later rendering (so that it renders with
                    // the number of pixels recommended by the qrcode library)
                    let ppp = ctx.pixels_per_point();

                    self.qr_codes.insert(
                        key.to_string(),
                        Ok((
                            texture_handle,
                            image.width() as f32 / ppp,
                            image.height() as f32 / ppp,
                        )),
                    );
                } else {
                    self.qr_codes.insert(
                        key.to_string(),
                        Err(("Could not make a QR", file!(), line!()).into()),
                    );
                }
            }
        }
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
            let mut cstate = egui::CollapsingState::load_with_default_open(ctx, *id, false);
            if item == SubMenu::Feeds || *submenu != SubMenu::Feeds {
                cstate.set_open(*submenu == item);
            }
            cstate.store(ctx);
        }
    }

    fn close_all_menus_except_feeds(&mut self, ctx: &Context) {
        for (submenu, id) in self.submenu_ids.iter() {
            let mut cstate = egui::CollapsingState::load_with_default_open(ctx, *id, false);
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
    ) -> (egui::CollapsingState, Response) {
        let mut cstate =
            egui::CollapsingState::load_with_default_open(ctx, self.submenu_ids[&submenu], false);
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

    fn after_openable_menu(&self, ui: &mut Ui, cstate: &egui::CollapsingState) {
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
        let too_rapid = Instant::now() - self.last_visible_update < Duration::from_secs(5);

        if no_change || scrolling || too_rapid {
            // Clear the accumulator
            // It will fill up again next frame and be tested again.
            self.next_visible_note_ids.clear();
            return;
        }

        // Update when this happened, so we don't accept again too rapidly
        self.last_visible_update = Instant::now();

        // Save to self.visibile_note_ids
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
    fn render_zap_area(&mut self, ui: &mut Ui, ctx: &Context) {
        let mut qr_string: Option<String> = None;

        match self.zap_state {
            ZapState::None => return, // should not occur
            ZapState::CheckingLnurl(_id, _pubkey, ref _lnurl) => {
                ui.label("Loading lnurl...");
            }
            ZapState::SeekingAmount(id, pubkey, ref _prd, ref _lnurl) => {
                let mut amt = 0;
                ui.label("Zap Amount:");
                if ui.button("1").clicked() {
                    amt = 1;
                }
                if ui.button("2").clicked() {
                    amt = 2;
                }
                if ui.button("5").clicked() {
                    amt = 5;
                }
                if ui.button("10").clicked() {
                    amt = 10;
                }
                if ui.button("21").clicked() {
                    amt = 21;
                }
                if ui.button("46").clicked() {
                    amt = 46;
                }
                if ui.button("100").clicked() {
                    amt = 100;
                }
                if ui.button("215").clicked() {
                    amt = 215;
                }
                if ui.button("464").clicked() {
                    amt = 464;
                }
                if ui.button("1000").clicked() {
                    amt = 1000;
                }
                if ui.button("2154").clicked() {
                    amt = 2154;
                }
                if ui.button("4642").clicked() {
                    amt = 4642;
                }
                if ui.button("10000").clicked() {
                    amt = 10000;
                }
                if amt > 0 {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::Zap(
                        id,
                        pubkey,
                        MilliSatoshi(amt * 1_000),
                        "".to_owned(),
                    ));
                }
                if ui.button("Cancel").clicked() {
                    *GLOBALS.current_zap.write() = ZapState::None;
                }
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
            self.render_qr(ui, ctx, "zap", &qr.to_uppercase());
            if ui.button("Close").clicked() {
                *GLOBALS.current_zap.write() = ZapState::None;
            }
        }
    }

    fn reset_draft(&mut self) {
        if let Page::Feed(FeedKind::DmChat(_)) = &self.page {
            self.dm_draft_data.clear();
        } else {
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
        ScrollArea::vertical().override_scroll_delta(Vec2 {
            x: 0.0,
            y: self.current_scroll_offset,
        })
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

                        let output = widgets::TextEdit::singleline(&app.theme, &mut app.password)
                            .password(true)
                            .with_paste()
                            .desired_width( 400.0)
                            .show_extended(ui, &mut app.clipboard);
                        if app.unlock_needs_focus {
                            output.response.request_focus();
                            app.unlock_needs_focus = false;
                        }

                        ui.add_space(20.0);

                        let mut submitted =
                            //response.lost_focus() &&
                            ui.input(|i| i.key_pressed(egui::Key::Enter));

                        ui.scope(|ui| {
                            app.theme.primary_button_style(ui.style_mut());
                            submitted |= ui.button("     Continue     ").clicked();
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
                                if ui.button("Delete Identity (Yes I'm Sure)").clicked() {
                                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::DeletePriv);
                                    app.delete_confirm = false;
                                    cancel_login();
                                }
                            } else {
                                if ui.button("Delete Identity (Cannot be undone!)").clicked() {
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
                                "Telegram Channel",
                                "https://t.me/gossipclient"
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
            ui.label("Please wait for the data migration to complete...");
        });
}
