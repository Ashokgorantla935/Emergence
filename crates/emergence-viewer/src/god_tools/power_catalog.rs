use super::mod_types::{ToolTab, PowerDef};

/// Complete catalog of all 78 god powers.
/// power IDs are indices 0..=77 in this array.
pub const POWER_CATALOG: &[PowerDef] = &[
    // ── Tab 1: Creation (12 powers) ──────────────────────────────────────────
    PowerDef { id: 0,  tab: ToolTab::Creation,    name: "Spawn Being",       shortcut: Some('1'), cooldown: 0,    tooltip: "Click to place a random being at cursor" },
    PowerDef { id: 1,  tab: ToolTab::Creation,    name: "Spawn Wanderer",    shortcut: Some('2'), cooldown: 0,    tooltip: "Bold explorer with high curiosity" },
    PowerDef { id: 2,  tab: ToolTab::Creation,    name: "Spawn Elder",       shortcut: Some('3'), cooldown: 0,    tooltip: "Wise, social, generous being" },
    PowerDef { id: 3,  tab: ToolTab::Creation,    name: "Spawn Bold",        shortcut: Some('4'), cooldown: 0,    tooltip: "Aggressive, courageous being" },
    PowerDef { id: 4,  tab: ToolTab::Creation,    name: "Spawn Pacifist",    shortcut: Some('5'), cooldown: 0,    tooltip: "Non-violent, generous being" },
    PowerDef { id: 5,  tab: ToolTab::Creation,    name: "Spawn Social",      shortcut: Some('6'), cooldown: 0,    tooltip: "Highly social, belonging-driven being" },
    PowerDef { id: 6,  tab: ToolTab::Creation,    name: "Spawn Bird",        shortcut: Some('7'), cooldown: 0,    tooltip: "Spawn a bird (fauna)" },
    PowerDef { id: 7,  tab: ToolTab::Creation,    name: "Spawn Deer",        shortcut: Some('8'), cooldown: 0,    tooltip: "Spawn a deer (fauna)" },
    PowerDef { id: 8,  tab: ToolTab::Creation,    name: "Spawn Wolf",        shortcut: Some('9'), cooldown: 0,    tooltip: "Spawn a wolf predator (fauna)" },
    PowerDef { id: 9,  tab: ToolTab::Creation,    name: "Spawn Rabbit",      shortcut: Some('0'), cooldown: 0,    tooltip: "Spawn a rabbit (fauna)" },
    PowerDef { id: 10, tab: ToolTab::Creation,    name: "Spawn Fish",        shortcut: None,      cooldown: 0,    tooltip: "Spawn fish near water" },
    PowerDef { id: 11, tab: ToolTab::Creation,    name: "Place Shelter",     shortcut: None,      cooldown: 0,    tooltip: "Mark a tile as a shelter" },

    // ── Tab 2: Terrain (10 powers) ────────────────────────────────────────────
    PowerDef { id: 12, tab: ToolTab::Terrain,     name: "Paint Grassland",   shortcut: Some('1'), cooldown: 0,    tooltip: "Brush: paint grassland biome" },
    PowerDef { id: 13, tab: ToolTab::Terrain,     name: "Paint Forest",      shortcut: Some('2'), cooldown: 0,    tooltip: "Brush: paint forest biome" },
    PowerDef { id: 14, tab: ToolTab::Terrain,     name: "Paint Desert",      shortcut: Some('3'), cooldown: 0,    tooltip: "Brush: paint desert biome" },
    PowerDef { id: 15, tab: ToolTab::Terrain,     name: "Paint Mountain",    shortcut: Some('4'), cooldown: 0,    tooltip: "Brush: paint mountain biome" },
    PowerDef { id: 16, tab: ToolTab::Terrain,     name: "Paint Wetland",     shortcut: Some('5'), cooldown: 0,    tooltip: "Brush: paint wetland biome" },
    PowerDef { id: 17, tab: ToolTab::Terrain,     name: "Raise Terrain",     shortcut: Some('6'), cooldown: 0,    tooltip: "Brush: raise elevation" },
    PowerDef { id: 18, tab: ToolTab::Terrain,     name: "Lower Terrain",     shortcut: Some('7'), cooldown: 0,    tooltip: "Brush: lower elevation" },
    PowerDef { id: 19, tab: ToolTab::Terrain,     name: "Create River",      shortcut: Some('8'), cooldown: 100,  tooltip: "Drag from source to destination" },
    PowerDef { id: 20, tab: ToolTab::Terrain,     name: "Create Lake",       shortcut: Some('9'), cooldown: 200,  tooltip: "Click to create a lake at cursor" },
    PowerDef { id: 21, tab: ToolTab::Terrain,     name: "Erase Water",       shortcut: Some('0'), cooldown: 50,   tooltip: "Brush: remove water from tiles" },

    // ── Tab 3: Weather (8 powers) ─────────────────────────────────────────────
    PowerDef { id: 22, tab: ToolTab::Weather,     name: "Rain",              shortcut: Some('1'), cooldown: 300,  tooltip: "Trigger rainfall in region" },
    PowerDef { id: 23, tab: ToolTab::Weather,     name: "Drought",           shortcut: Some('2'), cooldown: 500,  tooltip: "Trigger drought in region" },
    PowerDef { id: 24, tab: ToolTab::Weather,     name: "Storm",             shortcut: Some('3'), cooldown: 400,  tooltip: "Trigger storm in region" },
    PowerDef { id: 25, tab: ToolTab::Weather,     name: "Snow",              shortcut: Some('4'), cooldown: 300,  tooltip: "Trigger snowfall in region" },
    PowerDef { id: 26, tab: ToolTab::Weather,     name: "Heatwave",          shortcut: Some('5'), cooldown: 500,  tooltip: "Trigger heatwave in region" },
    PowerDef { id: 27, tab: ToolTab::Weather,     name: "Clear Weather",     shortcut: Some('6'), cooldown: 200,  tooltip: "Dispel active weather" },
    PowerDef { id: 28, tab: ToolTab::Weather,     name: "Set Spring",        shortcut: Some('7'), cooldown: 1000, tooltip: "Force season to Spring" },
    PowerDef { id: 29, tab: ToolTab::Weather,     name: "Set Summer",        shortcut: Some('8'), cooldown: 1000, tooltip: "Force season to Summer" },

    // ── Tab 4: Destruction (12 powers) ───────────────────────────────────────
    PowerDef { id: 30, tab: ToolTab::Destruction, name: "Lightning",         shortcut: Some('1'), cooldown: 60,   tooltip: "Strike with lightning, kills nearby beings" },
    PowerDef { id: 31, tab: ToolTab::Destruction, name: "Meteor",            shortcut: Some('2'), cooldown: 1200, tooltip: "Meteor strike: mass kill, crater terrain" },
    PowerDef { id: 32, tab: ToolTab::Destruction, name: "Earthquake",        shortcut: Some('3'), cooldown: 800,  tooltip: "Earthquake: damages and scatters beings" },
    PowerDef { id: 33, tab: ToolTab::Destruction, name: "Flood",             shortcut: Some('4'), cooldown: 600,  tooltip: "Flood a region with water" },
    PowerDef { id: 34, tab: ToolTab::Destruction, name: "Famine",            shortcut: Some('5'), cooldown: 400,  tooltip: "Remove all food from region" },
    PowerDef { id: 35, tab: ToolTab::Destruction, name: "Plague",            shortcut: Some('6'), cooldown: 500,  tooltip: "Spread illness in region" },
    PowerDef { id: 36, tab: ToolTab::Destruction, name: "Wildfire",          shortcut: Some('7'), cooldown: 200,  tooltip: "Ignite wildfire, burns food" },
    PowerDef { id: 37, tab: ToolTab::Destruction, name: "Tornado",           shortcut: Some('8'), cooldown: 700,  tooltip: "Tornado: scatter and frighten beings" },
    PowerDef { id: 38, tab: ToolTab::Destruction, name: "Kill Being",        shortcut: Some('9'), cooldown: 0,    tooltip: "Click to kill a specific being" },
    PowerDef { id: 39, tab: ToolTab::Destruction, name: "Kill Region",       shortcut: Some('0'), cooldown: 800,  tooltip: "Kill all beings in brush area" },
    PowerDef { id: 40, tab: ToolTab::Destruction, name: "Predator Pack",     shortcut: None,      cooldown: 600,  tooltip: "Spawn a pack of wolf predators" },
    PowerDef { id: 41, tab: ToolTab::Destruction, name: "Remove All",        shortcut: None,      cooldown: 1000, tooltip: "Remove all beings from region" },

    // ── Tab 5: Blessing (10 powers) ───────────────────────────────────────────
    PowerDef { id: 42, tab: ToolTab::Blessing,    name: "Bless Being",       shortcut: Some('1'), cooldown: 120,  tooltip: "Fill needs and boost joy of one being" },
    PowerDef { id: 43, tab: ToolTab::Blessing,    name: "Heal Being",        shortcut: Some('2'), cooldown: 60,   tooltip: "Restore needs for one being" },
    PowerDef { id: 44, tab: ToolTab::Blessing,    name: "Heal Region",       shortcut: Some('3'), cooldown: 300,  tooltip: "Partially restore needs of all beings in region" },
    PowerDef { id: 45, tab: ToolTab::Blessing,    name: "Inspire Courage",   shortcut: Some('4'), cooldown: 200,  tooltip: "Remove fear, boost curiosity in region" },
    PowerDef { id: 46, tab: ToolTab::Blessing,    name: "Inspire Calm",      shortcut: Some('5'), cooldown: 200,  tooltip: "Reduce anger and fear in region" },
    PowerDef { id: 47, tab: ToolTab::Blessing,    name: "Inspire Joy",       shortcut: Some('6'), cooldown: 150,  tooltip: "Boost joy, reduce grief in region" },
    PowerDef { id: 48, tab: ToolTab::Blessing,    name: "Love Spark",        shortcut: Some('7'), cooldown: 500,  tooltip: "Create strong bond between two beings" },
    PowerDef { id: 49, tab: ToolTab::Blessing,    name: "Feed Region",       shortcut: Some('8'), cooldown: 200,  tooltip: "Satisfy hunger of all beings in region" },
    PowerDef { id: 50, tab: ToolTab::Blessing,    name: "Extend Life",       shortcut: Some('9'), cooldown: 800,  tooltip: "Double lifespan of selected beings" },
    PowerDef { id: 51, tab: ToolTab::Blessing,    name: "Rejuvenate",        shortcut: Some('0'), cooldown: 600,  tooltip: "Reduce age by 2/3 for one being" },

    // ── Tab 6: Curse (10 powers) ──────────────────────────────────────────────
    PowerDef { id: 52, tab: ToolTab::Curse,       name: "Curse Being",       shortcut: Some('1'), cooldown: 120,  tooltip: "Drain needs and inflict grief on one being" },
    PowerDef { id: 53, tab: ToolTab::Curse,       name: "Madness",           shortcut: Some('2'), cooldown: 300,  tooltip: "Randomize personalities in region" },
    PowerDef { id: 54, tab: ToolTab::Curse,       name: "Isolation",         shortcut: Some('3'), cooldown: 200,  tooltip: "Make a being deeply antisocial" },
    PowerDef { id: 55, tab: ToolTab::Curse,       name: "Plague Curse",      shortcut: Some('4'), cooldown: 400,  tooltip: "Heavy sickness in region" },
    PowerDef { id: 56, tab: ToolTab::Curse,       name: "Aging Curse",       shortcut: Some('5'), cooldown: 300,  tooltip: "Age a being by 3 years instantly" },
    PowerDef { id: 57, tab: ToolTab::Curse,       name: "Hunger Curse",      shortcut: Some('6'), cooldown: 150,  tooltip: "Zero out hunger for one being" },
    PowerDef { id: 58, tab: ToolTab::Curse,       name: "Induce Rage",       shortcut: Some('7'), cooldown: 300,  tooltip: "Max anger in region" },
    PowerDef { id: 59, tab: ToolTab::Curse,       name: "Mark Hostile",      shortcut: Some('8'), cooldown: 400,  tooltip: "All nearby beings become angry at target" },
    PowerDef { id: 60, tab: ToolTab::Curse,       name: "Clear Memory",      shortcut: Some('9'), cooldown: 300,  tooltip: "Wipe causal memories of selected beings" },
    PowerDef { id: 61, tab: ToolTab::Curse,       name: "Modify Personality",shortcut: None,      cooldown: 600,  tooltip: "Shift a personality trait in selected beings" },

    // ── Tab 7: Kingdom (10 powers) ────────────────────────────────────────────
    PowerDef { id: 62, tab: ToolTab::Kingdom,     name: "Force Alliance",    shortcut: Some('1'), cooldown: 800,  tooltip: "Create positive bonds between two groups" },
    PowerDef { id: 63, tab: ToolTab::Kingdom,     name: "Force War",         shortcut: Some('2'), cooldown: 600,  tooltip: "Create hostility between two groups" },
    PowerDef { id: 64, tab: ToolTab::Kingdom,     name: "Revolution",        shortcut: Some('3'), cooldown: 700,  tooltip: "Trigger unrest and anger in region" },
    PowerDef { id: 65, tab: ToolTab::Kingdom,     name: "Teleport",          shortcut: Some('4'), cooldown: 100,  tooltip: "Click being, then click destination" },
    PowerDef { id: 66, tab: ToolTab::Kingdom,     name: "Exile",             shortcut: Some('5'), cooldown: 200,  tooltip: "Exile a being to a far destination" },
    PowerDef { id: 67, tab: ToolTab::Kingdom,     name: "Appoint Leader",    shortcut: Some('6'), cooldown: 400,  tooltip: "Boost social standing of one being" },
    PowerDef { id: 68, tab: ToolTab::Kingdom,     name: "Merge Settlements", shortcut: Some('7'), cooldown: 500,  tooltip: "Force trust between two settlement leaders" },
    PowerDef { id: 69, tab: ToolTab::Kingdom,     name: "Inspire Trade",     shortcut: Some('8'), cooldown: 400,  tooltip: "Boost warmth/trust between two groups" },
    PowerDef { id: 70, tab: ToolTab::Kingdom,     name: "Boost Loyalty",     shortcut: Some('9'), cooldown: 300,  tooltip: "Boost belonging and contentment in region" },
    PowerDef { id: 71, tab: ToolTab::Kingdom,     name: "Set Impressions",   shortcut: None,      cooldown: 400,  tooltip: "Manually set warmth/trust/debt between groups" },

    // ── Tab 8: World (8 powers) ───────────────────────────────────────────────
    PowerDef { id: 72, tab: ToolTab::World,       name: "Fast-Forward Year", shortcut: Some('1'), cooldown: 5000, tooltip: "Advance time by one in-game year" },
    PowerDef { id: 73, tab: ToolTab::World,       name: "Fast-Forward Season",shortcut: Some('2'), cooldown: 1500, tooltip: "Advance time by one season" },
    PowerDef { id: 74, tab: ToolTab::World,       name: "Snapshot Save A",   shortcut: Some('3'), cooldown: 0,    tooltip: "Save world snapshot to slot A" },
    PowerDef { id: 75, tab: ToolTab::World,       name: "Snapshot Save B",   shortcut: Some('4'), cooldown: 0,    tooltip: "Save world snapshot to slot B" },
    PowerDef { id: 76, tab: ToolTab::World,       name: "Snapshot Restore A",shortcut: Some('5'), cooldown: 0,    tooltip: "Restore world from slot A" },
    PowerDef { id: 77, tab: ToolTab::World,       name: "Snapshot Restore B",shortcut: Some('6'), cooldown: 0,    tooltip: "Restore world from slot B" },
];
