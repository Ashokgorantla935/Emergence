# Part 11: World News Feed & Commentary Timeline

**Depends on:** Part 5 (Observation Tools -- EventLog, Settlement Detector, Being Inspector), Part 1 (fix survival so events actually happen)

---

## Overview

A scrolling news feed that broadcasts significant world events as rich, human-readable messages. This is the player's CNN -- not a debug log. Only events worth knowing about surface here. A being ate food? Nobody cares. A kingdom collapsed? Front-page news.

The feed transforms raw `EventLog` entries into narrative text with clickable names, colored importance borders, and optional commentary on world trends.

---

## UI Layout

Semi-transparent panel, bottom-left of screen.

```
+--------------------------------------+
| WORLD NEWS                    [_][N] |  <- title bar, minimize, toggle
|--------------------------------------|
| [gold]  Day 142, Autumn, Year 3     |
| [crown] The Kingdom of Riverside    |
|         has been founded. Kira      |
|         rules 34 beings.           |
|                                      |
| [silver] Day 140, Autumn, Year 3    |
| [sword] Thane has become the        |
|         trusted leader of the River |
|         Settlement (trust: 0.82).   |
|                                      |
| [bronze] Day 138, Summer, Year 3    |  <- fades to 40% opacity
| [house] A hut has been built near   |
|         the river crossing.         |
|                                      |
|         ... older messages fade ...  |
+--------------------------------------+
```

### Dimensions & Positioning

| Property | Value |
|----------|-------|
| Width | 300px |
| Height | 200px (collapsed: 28px title bar only) |
| Position | bottom-left, 12px margin from screen edges |
| Background | `rgba(10, 10, 15, 0.75)` |
| Border | 1px `rgba(255, 255, 255, 0.15)` |
| Corner radius | 4px |
| Font | monospace, 12px body, 10px timestamp |
| Z-order | above world, below god tool palette, below inspector |

### Scroll Behavior

- Newest messages at top, feed scrolls downward
- Messages fade opacity from 100% (top) to 40% (bottom of visible area)
- Auto-scrolls to show newest message when a new one arrives
- Player can scroll manually (mouse wheel or drag). Manual scroll disables auto-scroll until player scrolls back to top
- Full history accessible by scrolling: last 500 messages retained

### Controls

| Input | Action |
|-------|--------|
| `N` key | Toggle panel visibility (collapsed/expanded) |
| Click message | Jump camera to event location (smooth pan, 0.3s ease) |
| Click being name | Select being in inspector |
| Click settlement name | Jump camera to settlement center |
| `Shift+N` | Open full history window (separate egui window, 600x400, searchable) |
| Right-click message | Pin message (stays visible at top, max 3 pins) |

---

## Event Categories & Importance Levels

### CRITICAL -- Gold Border (`#D4AF37`, 2px left border)

Always shown. Cannot be filtered out. These are world-shaping events.

| Event | Trigger Condition | Message Template |
|-------|-------------------|-----------------|
| Kingdom formed | Settlement reaches population >= 30 AND has a leader with trust >= 0.75 | `"The Kingdom of {kingdom_name} has been founded. {leader} rules {pop} beings."` |
| Kingdom fell | Kingdom leader dies AND no successor has trust >= 0.60 within 300 ticks | `"The Kingdom of {kingdom_name} has collapsed after {leader}'s death."` |
| War started | Two settlements with >= 15 beings each have average pairwise warmth < -0.4 AND 3+ theft/fight events between them in 600 ticks | `"Conflict erupts between {settlement1} and {settlement2} over {territory_description}."` |
| Mass death | 20+ beings die within 300 ticks in a 30-unit radius | `"A harsh {season} claimed {count} lives in {nearest_settlement_or_region}."` |
| First contact | Being from settlement A enters perception radius of being from settlement B, AND neither settlement has had prior contact | `"Settlers from {settlement1} have discovered the {settlement2} clan."` |
| Population milestone | Total alive beings crosses 1000, 2000, 5000, 10000 (each direction) | `"The world's population has reached {count} souls."` or `"The world's population has fallen to {count} souls."` |

### HIGH -- Silver Border (`#C0C0C0`, 2px left border)

Shown by default. Player can filter these out via settings.

| Event | Trigger Condition | Message Template |
|-------|-------------------|-----------------|
| Leader emerged | Being's average received trust from settlement members >= 0.70 AND settlement pop >= 8 | `"{name} has become the trusted leader of {settlement} (trust: {trust:.2})."` |
| Rebellion | 5+ beings with warmth < -0.3 toward leader AND at least one theft/fight against leader's allies within 600 ticks | `"{rebel_leader} leads {count} beings in revolt against {leader_title} {leader_name}."` |
| Settlement formed | Settlement detector registers new cluster (>= 5 beings, >= 600 ticks persistent) | `"A new settlement has formed near {landmark}. Population: {pop}."` |
| Settlement dissolved | Settlement population drops below 3 for 600+ ticks | `"{settlement} has been abandoned. Its last inhabitants scattered."` |
| Predator attack | Predator enters within 6 units of 3+ non-predator beings AND causes 1+ injury/death within 120 ticks | `"A wolf pack attacked {settlement_or_region}. {count} beings {injured_or_killed}."` |
| Famine | Settlement average hunger drops below 0.25 AND 5+ beings starving (hunger < 0.15) | `"Food supplies critically low in {settlement}. {count} beings starving."` |
| Peace restored | Two previously hostile settlements (warmth was < -0.3) rise above -0.1 average warmth | `"Tensions ease between {settlement1} and {settlement2} as traders restore warmth."` |
| God action (major) | Player uses Flood, Plague, Famine, or Predator Pack tool | `"A divine {event_type} strikes {region}."` |

### MEDIUM -- Bronze Border (`#CD7F32`, 1px left border)

Hidden by default. Shown when player clicks "Show All" or presses `Shift+N` for full history.

| Event | Trigger Condition | Message Template |
|-------|-------------------|-----------------|
| Birth of notable | Being born whose parent is a leader, elder, or has 10+ relationships | `"{child_name} was born to {parent1} and {parent2} in {settlement}."` |
| Elder death | Being dies with age >= 85% of lifespan AND is_notable | `"Elder {name} has died at age {age_years:.1} years. The settlement mourns."` |
| Bonding | Two beings reach warmth >= 0.85 AND trust >= 0.80 AND at least one is notable | `"{name1} and {name2} have bonded (warmth: {warmth:.2}, trust: {trust:.2})."` |
| Construction | Settlement builds a structure (hut detected: 3+ beings consistently within 2-unit radius near shelter terrain for 300+ ticks) | `"A hut has been built near {landmark}."` |
| Seasonal shift | Season changes (every 3600 ticks) | `"{season} has arrived. {flavor_text}."` |
| Migration | 5+ beings from same settlement move 40+ units from settlement center in the same direction within 600 ticks | `"A group of {count} beings from {settlement} is migrating {direction}."` |
| Large birth event | 3+ births in same settlement within 300 ticks | `"A baby boom in {settlement} -- {count} new beings this season."` |

**Seasonal flavor text:**

| Season | Flavor Text Options |
|--------|-------------------|
| Spring | `"Temperatures rise across the world."` / `"The land begins to thaw."` |
| Summer | `"Food grows plentiful in the forests."` / `"Long days stretch across the land."` |
| Autumn | `"Leaves fall. Food growth slows."` / `"The air grows cool."` |
| Winter | `"Temperatures drop across the world."` / `"Survival grows difficult."` |

### LOW -- No Border

Never shown in the feed panel. Only accessible in full history view (`Shift+N`).

| Event | Trigger Condition |
|-------|-------------------|
| Individual death (non-notable) | Being dies, not notable |
| Individual birth (non-notable) | Being born, neither parent notable |
| Resource depletion | Cell food drops below 0.1 (logged but never displayed individually) |
| Individual action milestone | Being performs 1000th action of a type (logged for analytics) |

---

## Notable Being Detection

A being is "notable" if ANY of the following are true:

| Criterion | Threshold | Rationale |
|-----------|-----------|-----------|
| Settlement leader | avg trust from settlement >= 0.70 | Political importance |
| High relationship count | 10+ unique beings with abs(warmth) >= 0.3 | Social hub |
| Elder | age >= 80% of lifespan | Wisdom/longevity |
| Involved in HIGH events | referenced in 3+ HIGH-level news messages | Historically important |
| God-placed | spawned by player (parent_ids both = u32::MAX) AND survived 3600+ ticks | Player investment |

**Name display rules:**

- Notable beings: always show procedural name. `"Kira"`, `"Elder Thane"`, `"Moss"`.
- Non-notable beings: show as `"a being"`, `"a settler"`, `"a young one"` (youth), `"a newcomer"` (age < 5% lifespan).
- Predators: `"a wolf"`, `"a wolf pack"` (3+).

**Notable tracking:**

```rust
struct NotableTracker {
    notable_set: HashSet<usize>,           // being indices currently notable
    event_counts: HashMap<usize, u16>,     // being_idx -> count of HIGH events referenced in
    check_interval: u32,                   // 600 ticks (once per game-day)
}
```

Re-evaluated every 600 ticks. Scan all alive beings against criteria. Cost: O(N) where N = alive beings. At 10K beings, ~10K comparisons = negligible.

---

## Message Generation

### Architecture

```
EventLog (raw events)
    |
    v
NewsFilter (importance check, O(1) per event)
    |
    v
MessageFormatter (template + variable substitution)
    |
    v
NewsFeed (ring buffer of 500 NewsMessage, renders via egui)
```

### NewsFilter

Subscribes to `EventLog`. On each new event, checks type against importance table:

```rust
fn classify_event(event: &WorldEvent, world: &World) -> Option<NewsImportance> {
    match event.event_type {
        EventType::SettlementFormed => {
            let pop = world.settlements[event.settlement_id].population;
            if pop >= 30 && has_high_trust_leader(event.settlement_id, world) {
                Some(NewsImportance::Critical)  // kingdom
            } else if pop >= 5 {
                Some(NewsImportance::High)       // settlement
            } else {
                None                             // too small, ignore
            }
        }
        EventType::Death => {
            if is_notable(event.being_id, world) {
                Some(NewsImportance::Medium)
            } else {
                Some(NewsImportance::Low)
            }
        }
        // ... etc
    }
}
```

Each event type maps to exactly one importance check. No dynamic scoring -- pure lookup + threshold. O(1) per event.

### MessageFormatter

Template-based string formatting with variable substitution.

```rust
struct NewsMessage {
    tick: u32,
    importance: NewsImportance,            // Critical, High, Medium, Low
    icon: NewsIcon,                        // Crown, Sword, Skull, Heart, House, Sun, etc.
    text: String,                          // formatted message, ~100-200 chars
    location: Option<[f32; 2]>,            // world position for camera jump
    referenced_beings: Vec<usize>,         // for click-to-inspect
    referenced_settlements: Vec<u32>,      // for click-to-jump
}

enum NewsIcon {
    Crown,      // kingdom events
    Sword,      // conflict, war, rebellion
    Skull,      // death, mass death, famine
    Heart,      // bonding, peace, birth
    House,      // settlement, construction
    Sun,        // seasonal, weather
    Lightning,  // god actions
    Footprints, // migration, first contact
    Star,       // population milestone
}
```

**Icon rendering:** 16x16 emoji-style glyphs rendered via egui. Each icon is a single Unicode character or a small texture atlas sprite (8 icons = 128x16 texture, negligible).

### Message Tone

Messages have personality. They read like a narrator, not a log file.

**Do:**
- `"The Kingdom of Riverside has collapsed after Kira's death."`
- `"A harsh winter claimed 47 lives in the northern settlements."`
- `"Tensions ease between Riverside and Hilltop as traders restore warmth."`

**Don't:**
- `"Settlement #4 dissolved at tick 84,201"`
- `"Being #2847 death event"`
- `"warmth(settlement[2], settlement[4]) > -0.1: peace"`

**Rich text formatting (egui RichText):**
- Being names: **bold**, clickable (blue underline on hover)
- Settlement names: **bold**, clickable (green underline on hover)
- Numbers (population, trust scores): monospace, white
- Timestamps: dim gray, 10px

---

## Commentary System (Toggleable)

Every 1800 ticks (half a season, ~3 minutes real-time at 10x speed), a commentary scan runs. It checks world state for statistical outliers and generates flavor text.

### Commentary Triggers

| Pattern | Detection | Example Message |
|---------|-----------|-----------------|
| Generous settlement | Settlement avg generosity > 0.6 | `"The beings of {settlement} seem unusually generous this season..."` |
| Rising tensions | 2+ settlements with avg warmth declining > 0.1 over last 3600 ticks | `"Tensions are rising in the {region}. {count} settlements share dwindling food."` |
| Long reign | Leader held position > 2 years (28,800 ticks) | `"{name} has been leader for {years} years -- the longest reign in the world."` |
| Population boom | Birth rate > 2x death rate over last 3600 ticks | `"Life flourishes. {births} new souls have arrived this season."` |
| Quiet world | No HIGH or CRITICAL events in last 3600 ticks | `"Peace settles over the world. For now."` |
| Loneliest being | Being with 0 relationships and age > 50% lifespan, notable | `"{name} wanders alone, far from any settlement."` |
| Old world | Average age > 60% of average lifespan | `"The world grows old. Few young ones remain."` |
| Trade network | 3+ settlements with positive avg warmth between all pairs | `"A web of trade connects {settlement1}, {settlement2}, and {settlement3}."` |

### Commentary Display

- Rendered in *italic*, no border, slightly different background tint (`rgba(40, 35, 20, 0.6)`)
- Icon: quill/scroll emoji
- Importance: always MEDIUM (shown in expanded view, never clutters default feed)
- Max 1 commentary per 1800-tick scan (pick the most interesting pattern)
- Toggleable: Settings > News Feed > "Show Commentary" checkbox (default: on)

### Commentary Scan Cost

One scan per 1800 ticks. Checks:
1. Settlement-level stats: O(S) where S = settlement count (typically < 20)
2. Leader tenure: O(S) -- one leader per settlement
3. Birth/death rates: already tracked in `StatisticsTracker`
4. Relationship density: sample 100 random beings, check relationship count

Total: < 200 operations per scan. Run on main thread, ~0.01ms. Not worth offloading.

---

## Rendering

### egui Implementation

```rust
struct NewsFeedPanel {
    messages: VecDeque<NewsMessage>,        // ring buffer, max 500
    pinned: Vec<usize>,                    // indices of pinned messages, max 3
    visible: bool,                         // toggled by N key
    auto_scroll: bool,                     // true until manual scroll
    filter_level: NewsImportance,          // default: High (show Critical + High)
    show_commentary: bool,                 // default: true
    scroll_offset: f32,                    // current scroll position
}
```

Rendered in `egui::Window` with `fixed_pos`, `fixed_size`, `no_title_bar` (custom title drawn manually for the minimize button).

Each message is an `egui::Frame` with:
- Left border colored by importance
- Icon + timestamp on first line
- Message body with rich text (clickable names)
- Opacity = `1.0 - (vertical_position / panel_height) * 0.6` (fades from 100% to 40%)

### Performance Budget

| Operation | Cost | Frequency |
|-----------|------|-----------|
| Event filtering | O(1) per event | Every event (~10-50/tick at peak) |
| Message formatting | String format, ~1us | Only filtered events (~1-5/day) |
| Commentary scan | ~200 ops, ~0.01ms | Every 1800 ticks |
| Rendering | egui ScrollArea, ~10 visible messages | Every frame |
| Total per frame | ~0.1ms | 60fps |

### Memory Budget

| Component | Size |
|-----------|------|
| 500 messages x ~200 bytes avg | ~100KB |
| Notable tracker (HashSet + HashMap) | ~80KB at 10K beings |
| Commentary state | ~1KB |
| **Total** | **~181KB** |

---

## Data Structures

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NewsImportance {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

#[derive(Clone, Copy)]
enum NewsIcon {
    Crown,
    Sword,
    Skull,
    Heart,
    House,
    Sun,
    Lightning,
    Footprints,
    Star,
    Quill,  // commentary
}

struct NewsMessage {
    tick: u32,
    importance: NewsImportance,
    icon: NewsIcon,
    text: String,
    location: Option<[f32; 2]>,
    referenced_beings: SmallVec<[usize; 4]>,
    referenced_settlements: SmallVec<[u32; 2]>,
    is_commentary: bool,
    pinned: bool,
}

struct NewsFeed {
    messages: VecDeque<NewsMessage>,  // cap 500, push_front, pop_back
    notable_tracker: NotableTracker,
    last_commentary_tick: u32,
    panel: NewsFeedPanel,
}
```

---

## Full History Window

Opened with `Shift+N`. Separate egui window, 600x400px, centered.

```
+--------------------------------------------------+
| WORLD HISTORY                              [X]   |
|--------------------------------------------------|
| [Search: ____________] [Filter: All v]           |
|--------------------------------------------------|
| [gold] Day 142, Autumn Y3                       |
|   The Kingdom of Riverside has been founded...   |
|                                                  |
| [silver] Day 140, Autumn Y3                     |
|   Thane has become the trusted leader...         |
|                                                  |
| [bronze] Day 138, Summer Y3                     |
|   A hut has been built near the river crossing.  |
|                                                  |
| (no border) Day 137, Summer Y3                   |
|   A being died near the eastern caves.           |
|                                                  |
|  ... scrollable, all 500 messages ...            |
+--------------------------------------------------+
```

**Search:** filters messages by substring match on text. Case-insensitive. Updates live as player types.

**Filter dropdown:** All | Critical Only | Critical + High | Critical + High + Medium (default matches panel filter level).

---

## Timestamp Format

All messages display time as `Day {day}, {season}, Year {year}` where:

```rust
fn format_timestamp(tick: u32) -> String {
    let day = tick / 600;                          // 600 ticks per day
    let year = day / 48 + 1;                       // 48 days per year (4 seasons x 12 days)
    let day_of_year = day % 48;
    let season = match day_of_year {
        0..=11 => "Spring",
        12..=23 => "Summer",
        24..=35 => "Autumn",
        _ => "Winter",
    };
    let day_num = day + 1;                         // 1-indexed for display
    format!("Day {day_num}, {season}, Year {year}")
}
```

---

## Territory & Landmark Naming

Messages reference locations. When no settlement exists nearby, use landmark descriptions:

```rust
fn describe_location(pos: [f32; 2], world: &World) -> String {
    // 1. Check if inside a settlement
    if let Some(s) = find_settlement_at(pos, world) {
        return s.name.clone();
    }
    // 2. Generate landmark description from terrain
    let biome = world.terrain.biome_at(pos);
    let direction = cardinal_from_center(pos, world.size);  // "northern", "eastern", etc.
    match biome {
        Biome::Forest => format!("the {} forests", direction),
        Biome::Water => format!("the {} river", direction),
        Biome::Mountain => format!("the {} mountains", direction),
        Biome::Desert => format!("the {} wastes", direction),
        Biome::Grassland => format!("the {} plains", direction),
    }
}
```

`cardinal_from_center()` divides the 256x256 map into 9 sectors (NW, N, NE, W, center, E, SW, S, SE) and returns the appropriate adjective.

---

## Integration Points

| System | Integration |
|--------|------------|
| `EventLog` | NewsFeed subscribes to EventLog. Each tick, drain new events through NewsFilter. |
| `SettlementDetector` | Used for settlement names, population, leader lookup. Already runs every 600 ticks. |
| `NotableTracker` | Updated every 600 ticks alongside settlement detection. Shares the same tick. |
| `Being Inspector` | Clicking a being name in a message selects that being in the inspector. |
| `Camera` | Clicking a message smoothly pans camera to `message.location`. |
| `StatisticsTracker` | Commentary system reads birth/death rates from existing stats ring buffer. |
| `God Tools` | God actions (Flood, Plague, etc.) emit events that NewsFeed formats as "divine intervention" messages. |

---

## Edge Cases

| Scenario | Handling |
|----------|---------|
| Being dies between event and click | Click does nothing. Tooltip: "This being is no longer alive." |
| Settlement dissolved between event and click | Jump to last known center position. Tooltip: "This settlement no longer exists." |
| Message flood (e.g., mass spawn via god tool) | Rate limit: max 5 messages per tick. Excess events of same type merged: "47 beings were placed by divine hand." |
| No events for long period | After 3600 ticks with no messages, inject commentary: "The world is quiet." |
| First tick / empty world | Show welcome message: "A new world awaits. Place beings to begin." (importance: Critical, icon: Star) |
