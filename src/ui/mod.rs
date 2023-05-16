macro_rules! text_edit_line {
    ($app:ident, $var:expr) => {
        egui::widgets::TextEdit::singleline(&mut $var)
            .text_color($app.settings.theme.input_text_color())
    };
}

macro_rules! text_edit_multiline {
    ($app:ident, $var:expr) => {
        egui::widgets::TextEdit::multiline(&mut $var)
            .text_color($app.settings.theme.input_text_color())
    };
}

mod components;
mod feed;
mod help;
mod people;
mod relays;
mod search;
mod settings;
mod theme;
mod widgets;
mod you;

use crate::about::About;
use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use crate::people::DbPerson;
use crate::settings::Settings;
pub use crate::ui::theme::{Theme, ThemeVariant};
use crate::ui::widgets::CopyButton;
#[cfg(feature = "video-ffmpeg")]
use core::cell::RefCell;
use eframe::{egui, IconData};
#[cfg(not(feature = "side-menu"))]
use egui::SelectableLabel;
use egui::{
    Color32, ColorImage, Context, Image, ImageData, Label, RichText, Sense, TextStyle,
    TextureHandle, TextureOptions, Ui, Vec2,
};
#[cfg(feature = "video-ffmpeg")]
use egui_video::{AudioDevice, Player};
use egui_winit::egui::Response;
use nostr_types::{Id, IdHex, Metadata, PublicKey, PublicKeyHex, RelayUrl, UncheckedUrl, Url};
use std::collections::{HashMap, HashSet};
#[cfg(feature = "video-ffmpeg")]
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

use self::widgets::NavItem;
use self::feed::Notes;

pub fn run() -> Result<(), Error> {
    let icon_bytes = include_bytes!("../../gossip.png");
    let icon = image::load_from_memory(icon_bytes)?.to_rgba8();
    let (icon_width, icon_height) = icon.dimensions();

    let options = eframe::NativeOptions {
        decorated: true,
        #[cfg(target_os = "macos")]
        fullsize_content: true,
        drag_and_drop_support: true,
        default_theme: if GLOBALS.settings.read().theme.dark_mode {
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
        follow_system_theme: GLOBALS.settings.read().theme.follow_os_dark_mode,
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
    Feed(FeedKind),
    PeopleList,
    PeopleFollow,
    PeopleMuted,
    Person(PublicKeyHex),
    YourKeys,
    YourMetadata,
    YourDelegation,
    RelaysActivityMonitor,
    RelaysKnownNetwork,
    Search,
    Settings,
    HelpHelp,
    HelpStats,
    HelpAbout,
}

#[derive(Eq, Hash, PartialEq)]
enum SubMenu {
    People,
    Relays,
    Account,
    Help,
}

impl SubMenu {
    fn to_id_str(&self) -> &'static str {
        match self {
            SubMenu::People => "people_submenu",
            SubMenu::Account => "account_submenu",
            SubMenu::Relays => "relays_submenu",
            SubMenu::Help => "help_submenu",
        }
    }
}

struct SubMenuState {
    submenu_states: HashMap<SubMenu, bool>,
}

impl SubMenuState {
    fn new() -> Self {
        let mut submenu_states: HashMap<SubMenu, bool> = HashMap::new();
        submenu_states.insert(SubMenu::People, false);
        submenu_states.insert(SubMenu::Relays, false);
        submenu_states.insert(SubMenu::Account, false);
        submenu_states.insert(SubMenu::Help, false);
        Self { submenu_states }
    }
    fn set_active(&mut self, item: &SubMenu) {
        for entry in self.submenu_states.iter_mut() {
            if entry.0 == item {
                *entry.1 = true;
            } else {
                *entry.1 = false;
            }
        }
    }
}

pub enum HighlightType {
    Nothing,
    PublicKey,
    Event,
}

struct GossipUi {
    #[cfg(feature = "video-ffmpeg")]
    audio_device: Option<AudioDevice>,
    #[cfg(feature = "video-ffmpeg")]
    video_players: HashMap<Url, Rc<RefCell<egui_video::Player>>>,

    // Rendering
    next_frame: Instant,
    override_dpi: bool,
    override_dpi_value: u32,
    current_scroll_offset: f32,
    future_scroll_offset: f32,

    // QR codes being rendered (in feed or elsewhere)
    // the f32's are the recommended image size
    qr_codes: HashMap<String, Result<(TextureHandle, f32, f32), Error>>,

    // Processed events caching
    notes: Notes,

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
    submenu_state: SubMenuState,

    // General Data
    about: About,
    icon: TextureHandle,
    placeholder_avatar: TextureHandle,
    settings: Settings,
    avatars: HashMap<PublicKeyHex, TextureHandle>,
    images: HashMap<Url, TextureHandle>,
    /// used when settings.show_media=false to explicitly show
    media_show_list: HashSet<Url>,
    /// used when settings.show_media=false to explicitly hide
    media_hide_list: HashSet<Url>,
    /// media that the user has selected to show full-width
    media_full_width_list: HashSet<Url>,

    // Search result
    search_result: String,

    // User entry: posts
    show_post_area: bool,
    draft: String,
    draft_needs_focus: bool,
    draft_repost: Option<Id>,
    unlock_needs_focus: bool,
    tag_someone: String,
    include_subject: bool,
    subject: String,
    include_content_warning: bool,
    content_warning: String,
    replying_to: Option<Id>,

    // User entry: metadata
    editing_metadata: bool,
    metadata: Metadata,

    // User entry: delegatee tag (as JSON string)
    delegatee_tag_str: String,

    // User entry: general
    nprofile_follow: String,
    nip05follow: String,
    follow_pubkey: String,
    follow_pubkey_at_relay: String,
    follow_clear_needs_confirm: bool,
    password: String,
    password2: String,
    password3: String,
    delete_confirm: bool,
    new_metadata_fieldname: String,
    import_priv: String,
    import_pub: String,
    new_relay_url: String,
    show_hidden_relays: bool,
    search: String,
    entering_search_page: bool,

    // Collapsed threads
    collapsed: Vec<Id>,
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
        let mut settings = GLOBALS.settings.read().clone();

        if let Some(dpi) = settings.override_dpi {
            let ppt: f32 = dpi as f32 / 72.0;
            cctx.egui_ctx.set_pixels_per_point(ppt);
            tracing::debug!("Pixels per point: {}", ppt);
        } else if let Some(ppt) = cctx.integration_info.native_pixels_per_point {
            cctx.egui_ctx.set_pixels_per_point(ppt);
            tracing::debug!("Pixels per point: {}", ppt);
        } else {
            tracing::debug!("Pixels per point: {}", cctx.egui_ctx.pixels_per_point());
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
        submenu_ids.insert(SubMenu::People, egui::Id::new(SubMenu::People.to_id_str()));
        submenu_ids.insert(
            SubMenu::Account,
            egui::Id::new(SubMenu::Account.to_id_str()),
        );
        submenu_ids.insert(SubMenu::Relays, egui::Id::new(SubMenu::Relays.to_id_str()));
        submenu_ids.insert(SubMenu::Help, egui::Id::new(SubMenu::Help.to_id_str()));

        let icon_texture_handle = {
            let bytes = include_bytes!("../../gossip.png");
            let image = image::load_from_memory(bytes).unwrap();
            let size = [image.width() as _, image.height() as _];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            cctx.egui_ctx.load_texture(
                "icon",
                ImageData::Color(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())),
                TextureOptions::default(), // magnification, minification
            )
        };

        let placeholder_avatar_texture_handle = {
            let bytes = include_bytes!("../../assets/placeholder_avatar.png");
            let image = image::load_from_memory(bytes).unwrap();
            let size = [image.width() as _, image.height() as _];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            cctx.egui_ctx.load_texture(
                "placeholder_avatar",
                ImageData::Color(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())),
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
        // let expand_right_symbol = {
        //     let bytes = include_bytes!("../../assets/expand-image.svg");
        //     let color_image = egui_extras::image::load_svg_bytes_with_size(
        //         bytes,
        //         egui_extras::image::FitTo::Size(200, 1000),
        //     ).unwrap();
        //     cctx.egui_ctx.load_texture(
        //         "expand_right_symbol",
        //         color_image,
        //         TextureOptions::default())
        // };

        let current_dpi = (cctx.egui_ctx.pixels_per_point() * 72.0) as u32;
        let (override_dpi, override_dpi_value): (bool, u32) = match settings.override_dpi {
            Some(v) => (true, v),
            None => (false, current_dpi),
        };

        let start_page = if GLOBALS.first_run.load(Ordering::Relaxed) {
            Page::HelpHelp
        } else {
            Page::Feed(FeedKind::Followed(false))
        };

        // Apply current theme
        if settings.theme.follow_os_dark_mode {
            settings.theme.dark_mode = cctx.egui_ctx.style().visuals.dark_mode;
        }
        theme::apply_theme(settings.theme, &cctx.egui_ctx);

        GossipUi {
            #[cfg(feature = "video-ffmpeg")]
            audio_device,
            #[cfg(feature = "video-ffmpeg")]
            video_players: HashMap::new(),
            next_frame: Instant::now(),
            override_dpi,
            override_dpi_value,
            current_scroll_offset: 0.0,
            future_scroll_offset: 0.0,
            qr_codes: HashMap::new(),
            notes: Notes::new(),
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
            submenu_state: SubMenuState::new(),
            about: crate::about::about(),
            icon: icon_texture_handle,
            placeholder_avatar: placeholder_avatar_texture_handle,
            settings,
            avatars: HashMap::new(),
            images: HashMap::new(),
            media_show_list: HashSet::new(),
            media_hide_list: HashSet::new(),
            media_full_width_list: HashSet::new(),
            search_result: "".to_owned(),
            show_post_area: false,
            draft: "".to_owned(),
            draft_needs_focus: false,
            draft_repost: None,
            unlock_needs_focus: false,
            tag_someone: "".to_owned(),
            include_subject: false,
            subject: "".to_owned(),
            include_content_warning: false,
            content_warning: "".to_owned(),
            replying_to: None,
            editing_metadata: false,
            metadata: Metadata::new(),
            delegatee_tag_str: "".to_owned(),
            nprofile_follow: "".to_owned(),
            nip05follow: "".to_owned(),
            follow_pubkey: "".to_owned(),
            follow_pubkey_at_relay: "".to_owned(),
            follow_clear_needs_confirm: false,
            password: "".to_owned(),
            password2: "".to_owned(),
            password3: "".to_owned(),
            delete_confirm: false,
            new_metadata_fieldname: String::new(),
            import_priv: "".to_owned(),
            import_pub: "".to_owned(),
            new_relay_url: "".to_owned(),
            show_hidden_relays: false,
            search: "".to_owned(),
            entering_search_page: false,
            collapsed: vec![],
        }
    }

    // maybe_relays is only used for Page::Feed(FeedKind::Thread...)
    fn set_page(&mut self, page: Page) {
        if self.page != page {
            tracing::trace!("PUSHING HISTORY: {:?}", &self.page);
            self.history.push(self.page.clone());
            self.set_page_inner(page);

            // Clear QR codes on page switches
            self.qr_codes.clear();
            self.render_qr = None;
            self.person_qr = None;
        }
    }

    fn set_page_thread_with_relays(&mut self, page: Page, relays: Vec<RelayUrl>) {
        tracing::debug!("RELAYS: {:?}", relays);
        if let Page::Feed(FeedKind::Thread { id, referenced_by }) = page {
            if self.page != page {
                tracing::trace!("PUSHING HISTORY: {:?}", &self.page);
                self.history.push(self.page.clone());

                GLOBALS.feed.set_feed_to_thread(id, referenced_by, relays);

                // Clear QR codes on page switches
                self.qr_codes.clear();
                self.render_qr = None;
                self.person_qr = None;

                self.page = page;
            }
        } else {
            self.set_page(page);
        }
    }

    fn back(&mut self) {
        if let Some(page) = self.history.pop() {
            tracing::trace!("POPPING HISTORY: {:?}", &page);
            self.set_page_inner(page);
        } else {
            tracing::trace!("HISTORY STUCK ON NONE");
        }
    }

    fn set_page_inner(&mut self, page: Page) {
        // Setting the page often requires some associated actions:
        match &page {
            Page::Feed(FeedKind::Followed(with_replies)) => {
                GLOBALS.feed.set_feed_to_followed(*with_replies);
            }
            Page::Feed(FeedKind::Inbox(indirect)) => {
                GLOBALS.feed.set_feed_to_inbox(*indirect);
            }
            Page::Feed(FeedKind::Thread { id, referenced_by }) => {
                GLOBALS.feed.set_feed_to_thread(*id, *referenced_by, vec![]);
            }
            Page::Feed(FeedKind::Person(pubkey)) => {
                GLOBALS.feed.set_feed_to_person(pubkey.to_owned());
            }
            Page::Search => {
                self.entering_search_page = true;
            }
            _ => {}
        }
        self.page = page;
    }

    fn clear_post(&mut self) {
        self.show_post_area = false;
        self.draft_needs_focus = false;
        self.draft = "".to_owned();
        self.draft_repost = None;
        self.tag_someone = "".to_owned();
        self.include_subject = false;
        self.subject = "".to_owned();
        self.replying_to = None;
        self.include_content_warning = false;
        self.content_warning = "".to_owned();
    }
}

impl eframe::App for GossipUi {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        let max_fps = GLOBALS.settings.read().max_fps as f32;

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
        }

        // Smooth Scrolling
        {
            // Add the amount of scroll requested to the future
            let mut requested_scroll: f32 = 0.0;
            ctx.input_mut(|i| {
                requested_scroll = i.scroll_delta.y;
            });
            self.future_scroll_offset += requested_scroll;

            // Move by 10% of future scroll offsets
            self.current_scroll_offset = 0.1 * self.future_scroll_offset;
            self.future_scroll_offset -= self.current_scroll_offset;

            // Friction stop when slow enough
            if self.future_scroll_offset < 1.0 && self.future_scroll_offset > -1.0 {
                self.future_scroll_offset = 0.0;
            }
        }

        if self.settings.theme.follow_os_dark_mode {
            // detect if the OS has changed dark/light mode
            let os_dark_mode = ctx.style().visuals.dark_mode;
            if os_dark_mode != self.settings.theme.dark_mode {
                // switch to the OS setting
                self.settings.theme.dark_mode = os_dark_mode;
                theme::apply_theme(self.settings.theme, ctx);
            }
        }

        #[cfg(not(feature = "side-menu"))]
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let back_label_text = RichText::new("‹ Back");
                let label = if self.history.is_empty() {
                    Label::new(back_label_text.weak())
                } else {
                    Label::new(back_label_text).sense(Sense::click())
                };
                if ui.add(label).clicked() {
                    self.back();
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        matches!(self.page, Page::Feed(_)),
                        "Feed",
                    ))
                    .clicked()
                {
                    self.set_page(Page::Feed(FeedKind::Followed(
                        self.mainfeed_include_nonroot,
                    )));
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::PeopleList
                            || self.page == Page::PeopleFollow
                            || self.page == Page::PeopleMuted
                            || matches!(self.page, Page::Person(_)),
                        "People",
                    ))
                    .clicked()
                {
                    self.set_page(Page::PeopleList);
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::YourKeys
                            || self.page == Page::YourMetadata
                            || self.page == Page::YourDelegation,
                        "You",
                    ))
                    .clicked()
                {
                    self.set_page(Page::YourKeys);
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::RelaysActivityMonitor || self.page == Page::RelaysKnownNetwork,
                        "Relays",
                    ))
                    .clicked()
                {
                    self.set_page(Page::RelaysActivityMonitor);
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(self.page == Page::Search, "Search"))
                    .clicked()
                {
                    self.set_page(Page::Search);
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::Settings,
                        "Settings",
                    ))
                    .clicked()
                {
                    self.set_page(Page::Settings);
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::HelpHelp
                            || self.page == Page::HelpStats
                            || self.page == Page::HelpAbout,
                        "Help",
                    ))
                    .clicked()
                {
                    self.set_page(Page::HelpHelp);
                }
                ui.separator();
            });
            ui.add_space(4.0);
        });

        #[cfg(feature = "side-menu")]
        egui::SidePanel::left("main-naviation-panel")
            .show_separator_line(false)
            .frame(
                egui::Frame::none()
                .inner_margin({
                    #[cfg(not(target_os = "macos"))]
                    let margin = egui::Margin::symmetric( 20.0, 20.0 );
                    #[cfg(target_os = "macos")]
                    let margin = egui::Margin{ left: 20.0, right: 20.0, top: 35.0, bottom: 20.0 };
                    margin
                })
                .fill(self.settings.theme.navigation_bg_fill())
            )
            .show(ctx, |ui| {
                    // cut indentation
                    ui.style_mut().spacing.indent = 0.0;
                    ui.style_mut().visuals.widgets.inactive.fg_stroke.color = self.settings.theme.navigation_text_color();
                    ui.style_mut().visuals.widgets.hovered.fg_stroke.color = self.settings.theme.navigation_text_hover_color();
                    ui.style_mut().visuals.widgets.hovered.fg_stroke.width = 1.0;
                    ui.style_mut().visuals.widgets.active.fg_stroke.color = self.settings.theme.navigation_text_active_color();

                    ui.add_space(4.0);
                    let back_label_text = RichText::new("‹ Back");
                    let label = if self.history.is_empty() {
                        Label::new(back_label_text.color(
                            Color32::from_white_alpha(8)))
                    } else {
                        Label::new(back_label_text.color(
                            self.settings.theme.navigation_text_color())
                        ).sense(Sense::click())
                    };
                    if ui.add(label).clicked() {
                        self.back();
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    if self.add_selected_label(
                            ui,
                            matches!(self.page, Page::Feed(FeedKind::Followed(_))),
                            "Main Feed",
                        )
                        .clicked()
                    {
                        self.set_page(Page::Feed(FeedKind::Followed(
                            self.mainfeed_include_nonroot,
                        )));
                    }
                    if let Some(pubkey) = GLOBALS.signer.public_key() {
                        let pubkeyhex: PublicKeyHex = pubkey.into();
                        if self.add_selected_label(
                                ui,
                                matches!(&self.page, Page::Feed(FeedKind::Person(key)) if key.as_str() == pubkeyhex.as_str()),
                                "My Notes",
                            )
                            .clicked()
                        {
                            self.set_page(Page::Feed(FeedKind::Person(pubkeyhex)));
                        }
                    }
                    if self.add_selected_label(
                            ui,
                            matches!(self.page, Page::Feed(FeedKind::Inbox(_))),
                            "Inbox",
                        )
                        .clicked()
                    {
                        self.set_page(Page::Feed(FeedKind::Inbox(self.inbox_include_indirect)));
                    }

                    ui.add_space(8.0);

                    // ---- People Submenu ----
                    {
                        let (mut submenu, header_response) =
                            self.get_openable_menu( ui, SubMenu::People, "People");
                        submenu.show_body_indented(&header_response, ui, |ui| {
                                self.add_menu_item_page(ui, Page::PeopleList, "Followed");
                                self.add_menu_item_page(ui, Page::PeopleFollow, "Follow new");
                                self.add_menu_item_page(ui, Page::PeopleMuted, "Muted");
                            });
                        self.after_openable_menu(ui, &submenu);
                    }
                    // ---- Relays Submenu ----
                    {
                        let (mut submenu, header_response) =
                            self.get_openable_menu( ui, SubMenu::Relays, "Relays");
                        submenu.show_body_indented(&header_response, ui, |ui| {
                                self.add_menu_item_page(ui, Page::RelaysActivityMonitor, "Activity Monitor");
                                self.add_menu_item_page(ui, Page::RelaysKnownNetwork, "Known Network");
                            });
                        self.after_openable_menu(ui, &submenu);
                    }
                    // ---- Account Submenu ----
                    {
                        let (mut submenu, header_response) =
                            self.get_openable_menu( ui, SubMenu::Account, "Account");
                        submenu.show_body_indented(&header_response, ui, |ui| {
                                self.add_menu_item_page(ui, Page::YourMetadata, "Profile");
                                self.add_menu_item_page(ui, Page::YourKeys, "Keys");
                                self.add_menu_item_page(ui, Page::YourDelegation, "Delegation");
                            });
                        self.after_openable_menu(ui, &submenu);
                    }
                    // ----
                    if self.add_selected_label(ui, self.page == Page::Search, "Search")
                        .clicked()
                    {
                        self.set_page(Page::Search);
                    }
                    // ----
                    if self.add_selected_label(
                            ui,
                            self.page == Page::Settings,
                            "Settings",
                        )
                        .clicked()
                    {
                        self.set_page(Page::Settings);
                    }
                    // ---- Help Submenu ----
                    {
                        let (mut submenu, header_response) =
                            self.get_openable_menu( ui, SubMenu::Help, "Help");
                        submenu.show_body_indented(&header_response, ui, |ui| {
                                self.add_menu_item_page(ui, Page::HelpHelp, "Help");
                                self.add_menu_item_page(ui, Page::HelpStats, "Stats");
                                self.add_menu_item_page(ui, Page::HelpAbout, "About");
                            });
                        self.after_openable_menu(ui, &submenu);
                    }

                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui|{
                        if ui.add(
                            Label::new(GLOBALS.status_message.blocking_read().clone())
                                .sense(Sense::click()),
                        )
                            .clicked()
                        {
                            *GLOBALS.status_message.blocking_write() = "".to_string();
                        }
                    });

                    // ---- "plus icon" ----
                    if !self.show_post_area {
                        let bottom_right = ui.ctx().screen_rect().right_bottom();
                        let pos = bottom_right + Vec2::new(-crate::AVATAR_SIZE_F32 * 2.0, -crate::AVATAR_SIZE_F32 * 2.0);

                        egui::Area::new(ui.next_auto_id())
                            .movable(false)
                            .interactable(true)
                            .fixed_pos(pos)
                            .constrain(true)
                            .show(ctx, |ui| {
                                egui::Frame::popup(&self.settings.theme.get_style())
                                    .rounding(egui::Rounding::same(crate::AVATAR_SIZE_F32/2.0)) // need the rounding for the shadow
                                    .stroke(egui::Stroke::NONE)
                                    .fill(Color32::TRANSPARENT)
                                    .shadow(egui::epaint::Shadow::NONE)
                                    .show(
                                    ui,
                                    |ui| {
                                        let text = if GLOBALS.signer.is_ready() {
                                            RichText::new("+").size(22.5)
                                        } else {
                                            RichText::new("\u{1f513}").size(20.0)
                                        };
                                        let response = ui.add_sized(
                                            [ crate::AVATAR_SIZE_F32 , crate::AVATAR_SIZE_F32 ],
                                            egui::Button::new( text
                                            .color(self.settings.theme.navigation_text_color()))
                                            .stroke(egui::Stroke::NONE)
                                            .rounding(egui::Rounding::same(crate::AVATAR_SIZE_F32))
                                            .fill(self.settings.theme.navigation_bg_fill()) );
                                        if response.clicked() {
                                            self.show_post_area = true;
                                            if GLOBALS.signer.is_ready() {
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

        #[cfg(feature = "side-menu")]
        egui::TopBottomPanel::top("top-area")
            .frame(
                egui::Frame::side_top_panel(&self.settings.theme.get_style()).inner_margin(
                    egui::Margin {
                        left: 20.0,
                        right: 15.0,
                        top: 10.0,
                        bottom: 10.0,
                    },
                ),
            )
            .resizable(true)
            .show_animated(
                ctx,
                self.show_post_area && self.settings.posting_area_at_top,
                |ui| {
                    feed::post::posting_area(self, ctx, frame, ui);
                },
            );

        #[cfg(not(feature = "side-menu"))]
        let show_status = true;
        #[cfg(feature = "side-menu")]
        let show_status = self.show_post_area && !self.settings.posting_area_at_top;

        #[cfg(not(feature = "side-menu"))]
        let resizable = false;
        #[cfg(feature = "side-menu")]
        let resizable = true;

        egui::TopBottomPanel::bottom("status")
            .frame({
                let frame = egui::Frame::side_top_panel(&self.settings.theme.get_style());
                #[cfg(feature = "side-menu")]
                let frame = frame.inner_margin(if !self.settings.posting_area_at_top {
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
                });
                frame
            })
            .resizable(resizable)
            .show_separator_line(false)
            .show_animated(ctx, show_status, |ui| {
                #[cfg(feature = "side-menu")]
                {
                    if self.show_post_area && !self.settings.posting_area_at_top {
                        ui.add_space(7.0);
                        feed::post::posting_area(self, ctx, frame, ui);
                    }
                }
                #[cfg(not(feature = "side-menu"))]
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            Label::new(GLOBALS.status_message.blocking_read().clone())
                                .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        *GLOBALS.status_message.blocking_write() = "".to_string();
                    }
                });
            });

        egui::CentralPanel::default()
            .frame({
                let frame = egui::Frame::central_panel(&self.settings.theme.get_style());
                #[cfg(feature = "side-menu")]
                let frame = frame.inner_margin(egui::Margin {
                    left: 20.0,
                    right: 10.0,
                    top: 10.0,
                    bottom: 0.0,
                });

                frame
            })
            .show(ctx, |ui| match self.page {
                Page::Feed(_) => feed::update(self, ctx, frame, ui),
                Page::PeopleList | Page::PeopleFollow | Page::PeopleMuted | Page::Person(_) => {
                    people::update(self, ctx, frame, ui)
                }
                Page::YourKeys | Page::YourMetadata | Page::YourDelegation => {
                    you::update(self, ctx, frame, ui)
                }
                Page::RelaysActivityMonitor | Page::RelaysKnownNetwork => relays::update(self, ctx, frame, ui),
                Page::Search => search::update(self, ctx, frame, ui),
                Page::Settings => settings::update(self, ctx, frame, ui),
                Page::HelpHelp | Page::HelpStats | Page::HelpAbout => {
                    help::update(self, ctx, frame, ui)
                }
            });
    }
}

impl GossipUi {
    /// A short rendering of a `PublicKey`
    pub fn pubkey_short(pk: &PublicKey) -> String {
        let npub = pk.as_bech32_string();
        format!("{}…", &npub.get(0..20).unwrap_or("????????????????????"))
    }

    /// A short rendering of a `PublicKeyHex`
    pub fn pubkeyhex_short(pubkeyhex: &PublicKeyHex) -> String {
        format!(
            "{}_{}...{}_{}",
            &pubkeyhex.as_str()[0..4],
            &pubkeyhex.as_str()[4..8],
            &pubkeyhex.as_str()[56..60],
            &pubkeyhex.as_str()[60..64],
        )
    }

    /// A short rendering of a `PublicKeyHex`, with attempt to convert to bech32
    pub fn pubkeyhex_convert_short(pubkeyhex: &PublicKeyHex) -> String {
        match PublicKey::try_from_hex_string(pubkeyhex) {
            Ok(pk) => Self::pubkey_short(&pk),
            Err(_) => GossipUi::pubkeyhex_short(pubkeyhex),
        }
    }

    pub fn hex_id_short(idhex: &IdHex) -> String {
        idhex.as_str()[0..8].to_string()
    }

    /// A display name for a `DbPerson`
    pub fn display_name_from_dbperson(dbperson: &DbPerson) -> String {
        if dbperson.muted == 1 {
            "{MUTED PERSON}".to_owned()
        } else {
            match dbperson.display_name() {
                Some(name) => name.to_owned(),
                None => Self::pubkeyhex_convert_short(&dbperson.pubkey),
            }
        }
    }

    /// A display name for a `PublicKeyHex`, via trying to lookup the person
    pub fn display_name_from_pubkeyhex_lookup(pkh: &PublicKeyHex) -> String {
        match GLOBALS.people.get(pkh) {
            Some(dbperson) => Self::display_name_from_dbperson(&dbperson),
            None => Self::pubkeyhex_convert_short(pkh),
        }
    }

    pub fn render_person_name_line(app: &mut GossipUi, ui: &mut Ui, person: &DbPerson) {
        // Let the 'People' manager know that we are interested in displaying this person.
        // It will make sure metadata is eventually available if
        // settings.automatically_fetch_metadata is enabled
        if person.metadata_at.is_none() {
            GLOBALS.people.person_of_interest(person.pubkey.clone());
        }

        ui.horizontal_wrapped(|ui| {
            let name = GossipUi::display_name_from_dbperson(person);

            ui.menu_button(&name, |ui| {
                let mute_label = if person.muted == 1 { "Unmute" } else { "Mute" };
                if ui.button(mute_label).clicked() {
                    GLOBALS.people.mute(&person.pubkey, person.muted == 0);
                    app.notes.cache_invalidate_person(&person.pubkey);
                }
                if person.followed == 0 && ui.button("Follow").clicked() {
                    GLOBALS.people.follow(&person.pubkey, true);
                } else if person.followed == 1 && ui.button("Unfollow").clicked() {
                    GLOBALS.people.follow(&person.pubkey, false);
                }
                if ui.button("Update Metadata").clicked() {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::UpdateMetadata(person.pubkey.clone()));
                }
                if ui.button("View Their Posts").clicked() {
                    app.set_page(Page::Feed(FeedKind::Person(person.pubkey.clone())));
                }
            });

            if person.followed > 0 {
                ui.label("🚶");
            }

            if let Some(mut nip05) = person.nip05().map(|s| s.to_owned()) {
                if nip05.starts_with("_@") {
                    nip05 = nip05.get(2..).unwrap().to_string();
                }

                if person.nip05_valid > 0 {
                    ui.label(RichText::new(nip05).monospace().small());
                } else {
                    ui.label(RichText::new(nip05).monospace().small().strikethrough());
                }
            }

            ui.label(RichText::new("🔑").text_style(TextStyle::Small).weak());
            if ui
                .add(CopyButton {})
                .on_hover_text("Copy Public Key")
                .clicked()
            {
                ui.output_mut(|o| o.copied_text = person.pubkey.as_bech32_string());
            }
        });
    }

    pub fn try_get_avatar(
        &mut self,
        ctx: &Context,
        pubkeyhex: &PublicKeyHex,
    ) -> Option<TextureHandle> {
        // Do not keep retrying if failed
        if GLOBALS.failed_avatars.blocking_read().contains(pubkeyhex) {
            return None;
        }

        if let Some(th) = self.avatars.get(pubkeyhex) {
            return Some(th.to_owned());
        }

        if let Some(color_image) = GLOBALS.people.get_avatar(pubkeyhex) {
            let texture_handle = ctx.load_texture(
                pubkeyhex.to_string(),
                color_image,
                TextureOptions::default(),
            );
            self.avatars
                .insert(pubkeyhex.to_owned(), texture_handle.clone());
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

        if let Some(color_image) = GLOBALS.media.get_image(&url) {
            let texture_handle =
                ctx.load_texture(url.0.clone(), color_image, TextureOptions::default());
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
                ui.add(Image::new(texture_handle, Vec2 { x: *x, y: *y }));
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
}

#[cfg(feature = "side-menu")]
impl GossipUi {
    fn add_menu_item_page(&mut self, ui: &mut Ui, page: Page, text: &str) {
        if self
            .add_selected_label(ui, self.page == page, text)
            .clicked()
        {
            self.set_page(page);
        }
    }

    fn get_openable_menu(
        &mut self,
        ui: &mut Ui,
        item: SubMenu,
        label: &str,
    ) -> (egui::CollapsingState, Response) {
        let mut clps =
            egui::CollapsingState::load_with_default_open(ui.ctx(), self.submenu_ids[&item], false);
        let txt = if clps.is_open() {
            label.to_string() + " \u{25BE}"
        } else {
            label.to_string() + " \u{25B8}"
        };
        if clps.is_open() {
            ui.add_space(10.0)
        }
        let header_res = ui.horizontal(|ui| {
            if ui
                .add(self.new_header_label(clps.is_open(), txt.as_str()))
                .clicked()
            {
                clps.toggle(ui);
                self.submenu_state.set_active(&item);
            }
        });
        if clps.is_open() {
            clps.set_open(self.submenu_state.submenu_states[&item]);
        }
        header_res
            .response
            .clone()
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        (clps, header_res.response)
    }

    fn after_openable_menu(&self, ui: &mut Ui, submenu: &egui::CollapsingState) {
        if submenu.is_open() {
            ui.add_space(10.0)
        }
    }

    fn new_header_label(&self, is_open: bool, text: &str) -> NavItem {
        NavItem::new(text, is_open)
            .color(self.settings.theme.navigation_text_color())
            .active_color(self.settings.theme.navigation_header_active_color())
            .hover_color(self.settings.theme.navigation_text_hover_color())
            .sense(Sense::click())
    }

    fn new_selected_label(&self, selected: bool, text: &str) -> NavItem {
        NavItem::new(text, selected)
            .color(self.settings.theme.navigation_text_color())
            .active_color(self.settings.theme.navigation_text_active_color())
            .hover_color(self.settings.theme.navigation_text_hover_color())
            .sense(Sense::click())
    }

    fn add_selected_label(&self, ui: &mut Ui, selected: bool, text: &str) -> egui::Response {
        let label = self.new_selected_label(selected, text);
        ui.add_space(2.0);
        let response = ui.add(label);
        ui.add_space(2.0);
        response
            .clone()
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response
    }
}
