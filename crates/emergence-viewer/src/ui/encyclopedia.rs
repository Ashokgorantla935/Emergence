/// G6 — In-Game Encyclopedia
/// E key toggles full-screen overlay.
/// 8 tabs. Entries unlock on first encounter.
/// Scrollable list per tab, search/filter at top.

use egui::{Color32, RichText};
use std::collections::HashSet;

// ── Unlock tracking ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct EncyclopediaProgress {
    pub unlocked: HashSet<String>,
}

impl EncyclopediaProgress {
    pub fn unlock(&mut self, key: &str) {
        self.unlocked.insert(key.to_string());
    }

    pub fn is_unlocked(&self, key: &str) -> bool {
        self.unlocked.contains(key)
    }
}

// ── Tabs ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EncycTab {
    Creatures,
    Emotions,
    Needs,
    Structures,
    GodPowers,
    WorldLaws,
    Personality,
    Kingdoms,
}

impl EncycTab {
    pub fn all() -> &'static [EncycTab] {
        &[
            EncycTab::Creatures,
            EncycTab::Emotions,
            EncycTab::Needs,
            EncycTab::Structures,
            EncycTab::GodPowers,
            EncycTab::WorldLaws,
            EncycTab::Personality,
            EncycTab::Kingdoms,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            EncycTab::Creatures   => "Creatures",
            EncycTab::Emotions    => "Emotions",
            EncycTab::Needs       => "Needs",
            EncycTab::Structures  => "Structures",
            EncycTab::GodPowers   => "God Powers",
            EncycTab::WorldLaws   => "World Laws",
            EncycTab::Personality => "Personality",
            EncycTab::Kingdoms    => "Kingdoms",
        }
    }
}

// ── Entry data ────────────────────────────────────────────────────────────────

struct Entry {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    stats: &'static str,
}

// ── Panel ─────────────────────────────────────────────────────────────────────

pub struct Encyclopedia {
    pub visible: bool,
    active_tab: EncycTab,
    search: String,
}

impl Encyclopedia {
    pub fn new() -> Self {
        Encyclopedia {
            visible: false,
            active_tab: EncycTab::Creatures,
            search: String::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn ui(&mut self, egui_ctx: &egui::Context, progress: &EncyclopediaProgress) {
        if !self.visible {
            return;
        }

        let screen = egui_ctx.screen_rect();
        let margin = screen.size() * 0.05;
        let window_rect = egui::Rect::from_min_size(
            screen.min + margin,
            screen.size() - margin * 2.0,
        );

        let mut open = true;
        egui::Window::new("Encyclopedia")
            .fixed_rect(window_rect)
            .open(&mut open)
            .show(egui_ctx, |ui| {
                // Search bar
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.text_edit_singleline(&mut self.search);
                    if ui.button("Clear").clicked() {
                        self.search.clear();
                    }
                });

                ui.separator();

                // Tab bar
                ui.horizontal(|ui| {
                    for &tab in EncycTab::all() {
                        let selected = self.active_tab == tab;
                        let btn = egui::Button::new(tab.label())
                            .selected(selected)
                            .min_size(egui::vec2(80.0, 26.0));
                        if ui.add(btn).clicked() {
                            self.active_tab = tab;
                        }
                    }
                });

                ui.separator();

                // Content
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.active_tab {
                        EncycTab::Creatures   => self.render_entries(ui, progress, creature_entries()),
                        EncycTab::Emotions    => self.render_entries(ui, progress, emotion_entries()),
                        EncycTab::Needs       => self.render_entries(ui, progress, need_entries()),
                        EncycTab::Structures  => self.render_entries(ui, progress, structure_entries()),
                        EncycTab::GodPowers   => self.render_entries(ui, progress, god_power_entries()),
                        EncycTab::WorldLaws   => self.render_entries(ui, progress, world_law_entries()),
                        EncycTab::Personality => self.render_entries(ui, progress, personality_entries()),
                        EncycTab::Kingdoms    => self.render_entries(ui, progress, kingdom_entries()),
                    }
                });
            });

        if !open {
            self.visible = false;
        }
    }

    fn render_entries(&self, ui: &mut egui::Ui, progress: &EncyclopediaProgress, entries: Vec<Entry>) {
        let filter = self.search.to_lowercase();
        let mut shown = 0;
        for entry in &entries {
            if !filter.is_empty() {
                let haystack = format!("{} {}", entry.name, entry.description).to_lowercase();
                if !haystack.contains(&filter) {
                    continue;
                }
            }
            shown += 1;
            let unlocked = progress.is_unlocked(entry.key);
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    if unlocked {
                        ui.label(RichText::new(entry.name).strong().color(Color32::WHITE));
                    } else {
                        ui.label(RichText::new("???").color(Color32::DARK_GRAY));
                    }
                });
                if unlocked {
                    ui.label(entry.description);
                    if !entry.stats.is_empty() {
                        ui.label(RichText::new(entry.stats).small().color(Color32::LIGHT_GRAY));
                    }
                } else {
                    ui.label(RichText::new("Encounter this to unlock.").color(Color32::DARK_GRAY).italics());
                }
            });
            ui.add_space(4.0);
        }
        if shown == 0 {
            ui.label("No entries match your search.");
        }
    }
}

// ── Entry data tables ─────────────────────────────────────────────────────────

fn creature_entries() -> Vec<Entry> {
    vec![
        Entry { key: "creature_human",  name: "Human",  description: "Emotionally complex social beings. Form settlements, bonds, and memories. Primary subject of the simulation.", stats: "Lifespan: 3-5 years | Social" },
        Entry { key: "creature_wolf",   name: "Wolf",   description: "Pack predator. Hunts weaker beings. Bold and social within its pack.", stats: "Lifespan: ~1.5 years | Predator" },
        Entry { key: "creature_deer",   name: "Deer",   description: "Docile grazer. Flees from predators. Important food source.", stats: "Lifespan: ~1.5 years | Prey" },
        Entry { key: "creature_rabbit", name: "Rabbit", description: "Small and fast. Breeds quickly. Common in grasslands and forests.", stats: "Lifespan: ~1 year | Prey" },
        Entry { key: "creature_fish",   name: "Fish",   description: "Aquatic creature. Found near water tiles. Fished by docks.", stats: "Lifespan: ~1 year | Aquatic" },
        Entry { key: "creature_hawk",   name: "Hawk",   description: "Aerial predator. Hunts rabbits and small creatures.", stats: "Lifespan: ~1.5 years | Aerial Predator" },
        Entry { key: "creature_bear",   name: "Bear",   description: "Apex predator. Solitary and powerful. Feared by settlements.", stats: "Lifespan: ~2 years | Apex Predator" },
        Entry { key: "creature_snake",  name: "Snake",  description: "Ambush predator. Found in forests and wetlands.", stats: "Lifespan: ~2 years | Ambush Predator" },
    ]
}

fn emotion_entries() -> Vec<Entry> {
    vec![
        Entry { key: "emotion_fear",      name: "Fear",      description: "Triggered by nearby predators, hunger, or danger signals. Causes flight. Spreads through proximity.", stats: "Decay: 0.02/tick | Color: Purple" },
        Entry { key: "emotion_joy",       name: "Joy",       description: "Arises from satiated needs, bonding, and positive memories. Spreads to nearby beings.", stats: "Decay: 0.015/tick | Color: Yellow" },
        Entry { key: "emotion_curiosity", name: "Curiosity", description: "Drives exploration and learning. High in beings with high Curious personality.", stats: "Decay: 0.01/tick | Color: Cyan" },
        Entry { key: "emotion_anger",     name: "Anger",     description: "Caused by unmet needs, betrayal, or rival groups. Leads to TakeFood action.", stats: "Decay: 0.02/tick | Color: Red" },
        Entry { key: "emotion_grief",     name: "Grief",     description: "Appears when a bonded being dies. Suppresses joy and curiosity.", stats: "Decay: 0.005/tick | Color: Blue" },
        Entry { key: "emotion_content",   name: "Contentment", description: "All major needs satisfied. Associated with settled, secure beings.", stats: "Decay: 0.01/tick | Color: Green" },
    ]
}

fn need_entries() -> Vec<Entry> {
    vec![
        Entry { key: "need_hunger",    name: "Hunger",    description: "Satisfaction = food consumed. Falls constantly. Below 0.2: death. Drives SeekFood.", stats: "Decay: 0.003/tick | Critical < 0.2" },
        Entry { key: "need_warmth",    name: "Warmth",    description: "Temperature comfort. Falls in cold biomes. Structures and campfires restore warmth.", stats: "Decay: 0.001/tick (climate-dependent)" },
        Entry { key: "need_safety",    name: "Safety",    description: "Proximity to threats lowers safety. Shelter, walls, and watchtowers help.", stats: "Restored by structures, reduced by predators" },
        Entry { key: "need_belonging", name: "Belonging", description: "Social connection. Rises when near bonded beings. Falls in isolation.", stats: "Key for settlement formation" },
        Entry { key: "need_purpose",   name: "Purpose",   description: "Drives construction and caregiving. High in social + curious beings.", stats: "Triggers Build action when low" },
        Entry { key: "need_rest",      name: "Rest",      description: "Falls over time. Restored by Sleep action. Night decreases rest decay.", stats: "Decay: 0.002/tick" },
    ]
}

fn structure_entries() -> Vec<Entry> {
    vec![
        Entry { key: "struct_campfire",   name: "Campfire",    description: "Provides warmth to beings in a 3-unit radius. Glows at night.", stats: "Cost: 0.2 | Build: 50t | Warmth +0.4" },
        Entry { key: "struct_leanto",     name: "Lean-To",     description: "Primitive shelter. Provides warmth and safety in a 2-unit radius.", stats: "Cost: 0.4 | Build: 100t | Warmth +0.3, Safety +0.3" },
        Entry { key: "struct_hut",        name: "Hut",         description: "Full shelter. Permanent home assignment for residents.", stats: "Cost: 0.8 | Build: 200t | Warmth +0.5, Safety +0.5, Belonging +0.3" },
        Entry { key: "struct_wall",       name: "Wall",        description: "Movement barrier for non-bonded beings and fauna.", stats: "Cost: 0.3 | Build: 80t | Decay: 8000t" },
        Entry { key: "struct_foodcache",  name: "Food Cache",  description: "Communal food storage. Holds up to 2.0 food units.", stats: "Cost: 0.1 | Build: 20t | Storage: 2.0" },
        Entry { key: "struct_watchtower", name: "Watchtower",  description: "Extends perception radius +4 units. Amplifies danger signals.", stats: "Cost: 0.6 | Build: 150t" },
        Entry { key: "struct_bridge",     name: "Bridge",      description: "Allows beings to cross water tiles.", stats: "Cost: 0.5 | Build: 120t" },
        Entry { key: "struct_farmplot",   name: "Farm Plot",   description: "Boosts food regrowth 10x in its tile.", stats: "Cost: 0.3 | Build: 80t | 3 growth stages" },
        Entry { key: "struct_dock",       name: "Dock",        description: "Boosts fishing yield 3x in adjacent water cells.", stats: "Cost: 0.5 | Build: 100t" },
        Entry { key: "struct_storagepit", name: "Storage Pit", description: "Large communal food bank. Holds up to 5.0 food units.", stats: "Cost: 0.4 | Build: 80t | Storage: 5.0" },
    ]
}

fn god_power_entries() -> Vec<Entry> {
    vec![
        Entry { key: "power_lightning",  name: "Lightning",     description: "Strikes a target area. Kills nearby beings, starts fires.", stats: "Tab: Destruction | Cooldown: 60t" },
        Entry { key: "power_meteor",     name: "Meteor Strike", description: "Massive destruction. Craters terrain.", stats: "Tab: Destruction | Cooldown: 1800t" },
        Entry { key: "power_joy",        name: "Joy Burst",     description: "Floods an area with joy emotion.", stats: "Tab: Blessing | Cooldown: 300t" },
        Entry { key: "power_plague",     name: "Plague",        description: "Spreads disease through a region, reducing needs over time.", stats: "Tab: Destruction | Cooldown: 3600t" },
        Entry { key: "power_lovespark",  name: "Love Spark",    description: "Forces two beings to form a bond immediately.", stats: "Tab: Blessing | Cooldown: 600t" },
        Entry { key: "power_amnesia",    name: "Amnesia",       description: "Wipes causal memory from selected beings.", stats: "Tab: Curse | Cooldown: 600t" },
        Entry { key: "power_fastfwd",    name: "Fast-Forward",  description: "Advances time by many ticks. Useful for watching long-term evolution.", stats: "Tab: World" },
        Entry { key: "power_earthquake", name: "Earthquake",    description: "Destabilizes terrain and structures across a region.", stats: "Tab: Destruction | Cooldown: 3600t" },
    ]
}

fn world_law_entries() -> Vec<Entry> {
    vec![
        Entry { key: "law_immortal",      name: "Immortal",          description: "Beings cannot die from old age. They can still be killed.", stats: "Category: Survival | Mutually exclusive with Fast Aging" },
        Entry { key: "law_infinite_food", name: "Infinite Food",     description: "Food cells are always full. No starvation possible.", stats: "Category: Environment" },
        Entry { key: "law_total_war",     name: "Total War",         description: "Anger toward non-settlement beings is always +0.3.", stats: "Category: Civilization | Mutually exclusive with Forced Peace" },
        Entry { key: "law_forced_peace",  name: "Forced Peace",      description: "Anger between settlements is always 0.", stats: "Category: Civilization | Mutually exclusive with Total War" },
        Entry { key: "law_no_memory",     name: "No Memory",         description: "Causal memories clear every 600 ticks.", stats: "Category: Social | Mutually exclusive with Perfect Memory" },
        Entry { key: "law_perfect_mem",   name: "Perfect Memory",    description: "Causal memories never decay.", stats: "Category: Social | Mutually exclusive with No Memory" },
        Entry { key: "law_fast_repro",    name: "Fast Reproduction", description: "Bond threshold halved, pregnancy shorter.", stats: "Category: Civilization | Mutually exclusive with No Reproduction" },
        Entry { key: "law_eternal_winter",name: "Eternal Winter",    description: "Season is permanently locked to Winter.", stats: "Category: Environment | Mutually exclusive with Eternal Spring" },
    ]
}

fn personality_entries() -> Vec<Entry> {
    vec![
        Entry { key: "trait_bold",      name: "Bold",      description: "High Bold beings initiate conflict, lead groups, and take risks. Low Bold beings avoid confrontation.", stats: "Range: -1 to +1 | Affects: TakeFood, leadership" },
        Entry { key: "trait_social",    name: "Social",    description: "High Social beings prioritize belonging and cluster with others. Low Social beings prefer solitude.", stats: "Range: -1 to +1 | Affects: Cluster, Bond actions" },
        Entry { key: "trait_curious",   name: "Curious",   description: "High Curious beings explore widely and accumulate more causal memories.", stats: "Range: -1 to +1 | Affects: Explore action frequency" },
        Entry { key: "trait_generous",  name: "Generous",  description: "High Generous beings share food readily. Low Generous (selfish) beings hoard.", stats: "Range: -1 to +1 | Affects: Share action frequency" },
        Entry { key: "trait_diurnal",   name: "Diurnal",   description: "High Diurnal beings are active during the day. Low Diurnal beings prefer the night.", stats: "Range: -1 to +1 | Affects: Sleep timing" },
    ]
}

fn kingdom_entries() -> Vec<Entry> {
    vec![
        Entry { key: "kingdom_formation",  name: "Kingdom Formation",  description: "Kingdoms form when 15+ beings cluster under a trusted leader (score >= 0.25). Settlements with mutual warmth > 0.3 merge.", stats: "Min population: 15 | Leader score = trust*0.7 + bold*0.15 + social*0.15" },
        Entry { key: "kingdom_succession", name: "Succession",         description: "When a leader dies, the settlement finds a new leader. If two candidates tie (within 0.10), the kingdom splits.", stats: "Hysteresis: challenger must exceed leader by 0.15" },
        Entry { key: "kingdom_war",        name: "War",                description: "Kingdoms go to war when average warmth between them falls below -0.3 or leader warmth < -0.4.", stats: "Visual: red pulsing border + red particle haze" },
        Entry { key: "kingdom_alliance",   name: "Alliance",           description: "Kingdoms ally when average warmth > 0.2 and leader warmth > 0.3.", stats: "Visual: green shared border + green line between capitals" },
        Entry { key: "kingdom_loyalty",    name: "Loyalty",            description: "Loyalty = belonging*0.3 + warmth_to_leader*0.35 + comfort*0.15 + safety*0.2. Low loyalty leads to rebellion.", stats: "Devoted >0.7 | Rebellious < -0.3" },
    ]
}
