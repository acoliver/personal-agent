use ksni::menu::StandardItem;
use ksni::{
    Category as KsniCategory, Icon as KsniIcon, MenuItem, Status as KsniStatus, Tray as KsniTray,
};
use std::sync::LazyLock;
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;

/// Decode the embedded menu-bar PNG into an ARGB32 `ksni::Icon` once at startup.
///
/// ksni's SNI protocol expects ARGB32 network-byte-order pixel data. The
/// `image` crate gives us RGBA8, so we rotate each 4-byte chunk right by one
/// (RGBA → ARGB). Using inline `icon_pixmap` instead of a freedesktop named
/// icon guarantees the tray renders even when the system icon theme lacks a
/// matching entry.
static TRAY_ICON: LazyLock<KsniIcon> = LazyLock::new(|| {
    let png_data = include_bytes!("../../../assets/MenuBarIcon.imageset/icon-32.png");
    let img = match image::load_from_memory_with_format(png_data, image::ImageFormat::Png) {
        Ok(img) => img,
        Err(error) => {
            tracing::error!(
                ?error,
                "Failed to decode embedded tray icon PNG; using 1x1 placeholder"
            );
            return KsniIcon {
                width: 1,
                height: 1,
                data: vec![0xFF, 0x00, 0x00, 0x00],
            };
        }
    };
    let (width, height) = (img.width() as i32, img.height() as i32);
    let mut data = img.into_rgba8().into_vec();
    for pixel in data.chunks_exact_mut(4) {
        pixel.rotate_right(1);
    }
    tracing::info!(width, height, "Decoded embedded tray icon for ksni");
    KsniIcon {
        width,
        height,
        data,
    }
});

#[derive(Debug, Clone, Copy)]
pub(super) enum LinuxTrayEvent {
    Activate { x: i32, y: i32 },
    OpenPopup,
    Quit,
}

pub(super) struct LinuxTray {
    pub(super) click_tx: UnboundedSender<LinuxTrayEvent>,
}

impl KsniTray for LinuxTray {
    fn id(&self) -> String {
        "com.personalagent.gpui".to_string()
    }

    fn title(&self) -> String {
        "PersonalAgent".to_string()
    }

    fn category(&self) -> KsniCategory {
        KsniCategory::ApplicationStatus
    }

    fn status(&self) -> KsniStatus {
        KsniStatus::Active
    }

    fn icon_name(&self) -> String {
        "personal-agent".to_string()
    }

    fn icon_pixmap(&self) -> Vec<KsniIcon> {
        vec![TRAY_ICON.clone()]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "PersonalAgent".to_string(),
            description: "Click to open chat".to_string(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let tx_open = self.click_tx.clone();
        let tx_quit = self.click_tx.clone();
        vec![
            MenuItem::Standard(StandardItem {
                label: "Open".to_string(),
                activate: Box::new(move |_this| {
                    let _ = tx_open.send(LinuxTrayEvent::OpenPopup);
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "Quit".to_string(),
                activate: Box::new(move |_this| {
                    let _ = tx_quit.send(LinuxTrayEvent::Quit);
                }),
                ..Default::default()
            }),
        ]
    }

    fn activate(&mut self, x: i32, y: i32) {
        let _ = self.click_tx.send(LinuxTrayEvent::Activate { x, y });
    }

    fn secondary_activate(&mut self, x: i32, y: i32) {
        let _ = self.click_tx.send(LinuxTrayEvent::Activate { x, y });
    }

    fn watcher_online(&self) {
        info!("Linux SNI watcher online");
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        info!(?reason, "Linux SNI watcher offline");
        true
    }
}

/// Detect the taskbar/panel height through the X root window's work area.
pub(super) fn taskbar_height(screen_height: f32) -> f32 {
    if let Ok(output) = std::process::Command::new("xprop")
        .args(["-root", "_NET_WORKAREA"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(work_area_h) = text
                .split('=')
                .nth(1)
                .and_then(|value| value.trim().trim_end_matches(',').split(',').nth(3))
                .and_then(|value| value.trim().parse::<f32>().ok())
            {
                let taskbar = screen_height - work_area_h;
                if taskbar > 0.0 && taskbar < 200.0 {
                    return taskbar;
                }
            }
        }
    }
    56.0
}
