//! Cog names and their display labels, ported from `classifyCog`,
//! `parseSmallCog` and `cogDisplayName` in `js/cogs/cog-board.js`, and from
//! `cogStatTier` in `js/cogs/cog-judge.js`.

/// Which family a cog name belongs to, for labels and colours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CogKind {
    Empty,
    Player(String),
    Yellow,
    Excogia { piece: usize },
    Crystal { level: usize },
    Standard { tier: usize, variant: String },
    Small { kind: usize, level: usize },
    Unknown,
}

/// Classify a cog name the way `classifyCog` does upstream.
pub fn classify(name: &str) -> CogKind {
    if name.is_empty() || name == "Blank" {
        return CogKind::Empty;
    }
    if let Some(character) = name.strip_prefix("Player_") {
        return CogKind::Player(character.to_string());
    }
    if name == "CogY" {
        return CogKind::Yellow;
    }
    if name.starts_with("CogZA") {
        let piece = name
            .chars()
            .nth(5)
            .and_then(|ch| ch.to_digit(10))
            .unwrap_or(0) as usize;
        return CogKind::Excogia { piece };
    }
    if name.starts_with("CogCry") {
        let level = name
            .chars()
            .nth(6)
            .and_then(|ch| ch.to_digit(10))
            .unwrap_or(0) as usize;
        return CogKind::Crystal { level };
    }
    if let Some(rest) = name.strip_prefix("Cog") {
        let mut characters = rest.chars();
        if let Some(tier) = characters.next().and_then(|ch| ch.to_digit(10)) {
            let variant: String = characters.collect();
            if !variant.is_empty() {
                return CogKind::Standard {
                    tier: tier as usize,
                    variant,
                };
            }
        }
    }
    if name.starts_with("CogSm") {
        if let Some((kind, level)) = small_cog(name) {
            return CogKind::Small { kind, level };
        }
        return CogKind::Small { kind: 0, level: 0 };
    }
    CogKind::Unknown
}

/// A small cog's type and level, as `parseSmallCog` reads them.
///
/// `CogSm_` is type 0 (Flaggy, ×2), `CogSma` type 1 (build, ×4) and `CogSmb`
/// type 2 (construction EXP, ×1).
pub(crate) fn small_cog(name: &str) -> Option<(usize, usize)> {
    let rest = name.strip_prefix("CogSm")?;
    let type_char = rest.chars().next()?;
    let kind = "_abcdefghijklmnopqrstuvwxyz".find(type_char)?;
    let level = rest
        .get(type_char.len_utf8()..)
        .and_then(|text| text.parse::<usize>().ok())
        .unwrap_or(0);
    Some((kind, level))
}

/// The stat-roll tier of a cog name, from `cogStatTier`: a standard cog's own
/// tier 0-3, or 4 for every crystal cog (the `CogCry` number is its upgrade
/// level, not its stat tier). `None` for a cog that does not roll stats.
pub(crate) fn stat_tier(name: &str) -> Option<usize> {
    match classify(name) {
        CogKind::Crystal { .. } => Some(4),
        CogKind::Standard { tier, .. } => Some(tier),
        _ => None,
    }
}

/// The display name from `cogDisplayName` upstream.
pub fn display_name(name: &str) -> String {
    const CRYSTALS: [&str; 6] = ["Topaz", "Ruby", "Amethyst", "Garnet", "Emerald", "Bluegem"];
    const TIERS: [&str; 4] = ["Nooby", "Decent", "Superb", "Ulti"];
    if name.is_empty() || name == "Blank" {
        return "Empty".into();
    }
    if let Some(character) = name.strip_prefix("Player_") {
        return character.to_string();
    }
    if name == "CogY" {
        return "Yang Cog".into();
    }
    if name.starts_with("CogZA") {
        return "Yin Cog".into();
    }
    if let Some(level) = name.strip_prefix("CogCry") {
        let index = level
            .chars()
            .next()
            .and_then(|ch| ch.to_digit(10))
            .unwrap_or(0) as usize;
        return format!("{} Cog", CRYSTALS.get(index).copied().unwrap_or("Crystal"));
    }
    if let Some(rest) = name.strip_prefix("Cog") {
        let mut characters = rest.chars();
        if let Some(tier) = characters.next().and_then(|ch| ch.to_digit(10)) {
            let suffix: String = characters.collect();
            if !suffix.is_empty() {
                let variant = variant_name(&suffix);
                let prefix = TIERS.get(tier as usize).copied().unwrap_or("Tier");
                return if variant.is_empty() {
                    format!("{prefix} Cog")
                } else {
                    format!("{prefix} {variant} Cog")
                };
            }
        }
    }
    if let Some((kind, level)) = small_cog(name) {
        let kinds = ["Flaggy Rate", "Build Speed", "Construction EXP"];
        return format!(
            "{} Tiny Cog T{}",
            kinds.get(kind).copied().unwrap_or("Tiny"),
            level + 1
        );
    }
    name.to_string()
}

/// The upstream `VARIANT_NAMES` table, with the raw suffix as the fallback.
fn variant_name(suffix: &str) -> String {
    match suffix {
        "A0" | "A00" => String::new(),
        "A1" => "Average".into(),
        "A2" => "Spur".into(),
        "A3" => "Stacked".into(),
        "A4" => "Deckered".into(),
        "B0" => "Double".into(),
        "B1" => "Trips".into(),
        "B2" => "Trabble".into(),
        "B3" => "Quad".into(),
        "B4" => "Penta".into(),
        "ad" => "Adjay".into(),
        "di" => "Diggle".into(),
        "le" => "Leff".into(),
        "ri" => "Rite".into(),
        "up" => "Uppy".into(),
        "do" => "Downer".into(),
        "cn" => "Sniper".into(),
        "co" => "Collumm".into(),
        "ro" => "Rowow".into(),
        "cr" => "Omni".into(),
        other => other.to_string(),
    }
}
