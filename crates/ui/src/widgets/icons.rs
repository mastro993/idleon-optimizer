//! The icon source: the kit's component icons and the Lucide icons the app adds.
//!
//! `gpui_kit::assets::Assets` embeds only the icons the kit's own components
//! draw. Any other `gpui_kit::assets::IconName` renders blank until its SVG is
//! embedded here, so a screen that needs a new icon adds it to the list below.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use gpui_kit::assets::{Assets, icon_assets};

icon_assets!(
    Extra,
    [
        CalendarRange,
        Download,
        GitBranch,
        House,
        ImageOff,
        ListClock,
        RotateCcw,
        Save,
        SearchX,
        SkipBack,
        SkipForward,
        Sparkles,
        Trash,
        Upload,
        X,
        Zap,
    ]
);

/// Serves the app's extra icons, then the kit's component icons.
#[derive(Clone, Copy, Debug, Default)]
pub struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match Extra.load(path)? {
            Some(data) => Ok(Some(data)),
            None => Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = Extra.list(path)?;
        paths.extend(Assets.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
