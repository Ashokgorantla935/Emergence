# Part 9: Emergent Warfare, Diplomacy, God Powers & World Laws

**Depends on:** Parts 1-7 (engine fixes, god tools, visuals, scenarios, observation, sound, ecosystem)

---

## 9.1 Emergent Warfare -- No Programmed Armies

There are no army units, no attack commands, no war declarations in the engine. Warfare emerges from the same personality/need/signal/relationship systems that drive everything else. What the player sees as "war" is bold, angry, hungry beings making individually rational decisions that happen to align.

### Raiding

A raid is NOT a scripted event. It is what you see when multiple conditions converge:

**Preconditions for a being to raid:**
1. `personality[BOLD] > 0.4` -- only assertive beings leave safety
2. `emotions[EMO_ANGER] > 0.3` toward at least one being in another settlement (from past theft, territory encroachment, witnessed aggression)
3. `combat_modifier > 0.0` -- the being has picked up or been given a weapon (stone, stick, crafted tool via the resource system)
4. `needs[NEED_HUNGER] < 0.4` OR `needs[NEED_BELONGING] < 0.3` -- driven by desperation (hungry) or alienation (outcast)

**How it happens:**

```
Being A is hungry (0.3), bold (0.7), angry at settlement B (warmth = -0.5 toward B's members).
Being A's action scoring:
  - SeekFood: score 0.6 (hungry, but local food depleted)
  - TakeFood: score 0.85 (hungry + bold + anger toward target + armed)
    Target: nearest being from settlement B carrying food, or B's food-rich territory
  - Flee: score 0.0 (bold overrides fear)

Being A moves toward settlement B.
```

Meanwhile, Being C and Being D in settlement A have similar profiles -- hungry, bold, angry at B. They independently score TakeFood highest and move in the same direction. The player sees three beings moving toward settlement B in loose formation. This is a raid.

**Raid detection (viewer only, no engine concept):**

```rust
// In swarm-viewer, every 600 ticks (1 game-day):
fn detect_raids(beings: &Beings, settlements: &[Settlement]) -> Vec<RaidEvent> {
    let mut raids = Vec::new();
    // Group beings by: (home_settlement, target_settlement)
    // where target = settlement containing their TakeFood target
    // If 3+ beings from settlement A are moving toward settlement B
    // with TakeFood as active action, emit RaidEvent
    for (home, target, raiders) in grouped_movements {
        if raiders.len() >= 3 {
            raids.push(RaidEvent {
                attackers: raiders,
                source: home,
                target,
                tick: current_tick,
            });
        }
    }
    raids
}
```

The viewer labels this: **"Raiders from [A] approaching [B]"** in the event log. The beings don't know they're "raiding." They're individually seeking food from a place they're angry at.

### Collective Defense

Defense is also emergent. When raiders enter a settlement's territory:

1. **Danger signal spikes** -- raiders deposit aggression/danger signals as they move aggressively (existing signal system)
2. **Bonded beings react** -- beings with high warmth toward threatened settlement members score `ApproachBeing` highly (social protection instinct). Bold beings with warmth > 0.5 toward endangered beings move toward them.
3. **Clustering under threat** -- social beings (social > 0.4) cluster when danger signal > 0.2. This is the existing Cluster action scoring higher when danger is present. The result looks like defenders grouping up.
4. **Counter-attack** -- if a defender has bold > 0.5, combat_modifier > 0, and anger toward a raider (from witnessing aggression), they score TakeFood/ApproachBeing toward the raider. Defenders fight raiders.

**No guard duty, no walls, no garrison.** Defense is: social beings clustering near bonded beings, bold beings confronting threats. Timid beings flee. The settlement's response depends entirely on the personalities of its inhabitants.

### Combat Resolution

Combat happens when two hostile beings are within 1.5 units of each other and at least one has TakeFood as their active action:

```rust
fn resolve_combat(attacker: usize, defender: usize, beings: &mut Beings) {
    let atk_power = beings.combat_modifier[attacker]
        * (0.5 + 0.5 * beings.personality[attacker][BOLD])
        * (0.8 + 0.2 * beings.needs[attacker][NEED_HUNGER].min(0.5) * 2.0); // desperation bonus

    let def_power = beings.combat_modifier[defender]
        * (0.5 + 0.5 * beings.personality[defender][BOLD])
        * if beings.current_action[defender] == Action::Flee { 0.3 } else { 1.0 }; // fleeing = weak

    // Probabilistic: higher power = higher chance of landing a hit
    let hit_chance = atk_power / (atk_power + def_power + 0.1);

    if rng.f32() < hit_chance {
        // Attacker lands hit
        let damage = 0.15 * atk_power; // hunger/health damage
        beings.needs[defender][NEED_HUNGER] = (beings.needs[defender][NEED_HUNGER] - damage).max(0.0);
        beings.emotions[defender][EMO_FEAR] = (beings.emotions[defender][EMO_FEAR] + 0.3).min(1.0);
        beings.emotions[defender][EMO_ANGER] = (beings.emotions[defender][EMO_ANGER] + 0.2).min(1.0);

        // Witnessing: all beings within 6 units witness this
        // Bold witnesses: anger toward attacker increases
        // Timid witnesses: fear increases, may flee
        // Social witnesses bonded to defender: anger toward attacker spikes

        deposit_signal(SignalChannel::Danger, pos, 0.8);

        // Causal memory: defender remembers attacker as threat
        update_impression(defender, attacker, WARMTH, -0.4);
        update_impression(defender, attacker, TRUST, -0.6);
    }

    // Defender can counter-attack on same tick if not fleeing
    if beings.current_action[defender] != Action::Flee {
        // mirror logic with def as attacker
    }
}
```

**Death in combat:** when hunger reaches 0 from combat damage, the being enters starvation (same as hunger death, but combat deaths trigger stronger grief signals -- strength 1.5 vs 0.5 for starvation death). Combat kills are logged in causal memory of all witnesses.

### Siege Dynamics

When raiders occupy territory for extended periods (>300 ticks in another settlement's area), the following emerges:

1. **Comfort signal collapses** -- raiders deposit danger, suppressing comfort. Beings in the area lose belonging satisfaction.
2. **Food depletion** -- raiders consume local food (TakeFood action), depleting the area faster than regrowth.
3. **Population displacement** -- timid beings (bold < 0.0) flee when danger signal stays above 0.3 for >100 ticks. They move away from danger toward comfort, often ending up at the settlement's periphery or in the wilderness.
4. **Infrastructure decay** -- with inhabitants displaced, no one maintains shelters. Shelter signal fades. Warmth satisfaction drops for remaining beings.

The viewer detects siege when:
- 3+ raiders from settlement A have been within settlement B's territory for >300 ticks
- At least 2 of B's inhabitants have fled (moved >20 units from settlement center)

Label: **"[A] raiders besieging [B]"**

### Peace-Making

Peace is as emergent as war. The mechanism: **generous beings as bridge-builders.**

When a generous being (generous > 0.5) from settlement A encounters a being from hostile settlement B:

1. If the A-being is not currently angry at the B-being (anger < 0.2), they score `ShareFood` or `ApproachBeing` toward the B-being
2. Sharing food deposits positive warmth (+0.3) in the B-being's impression of the A-being
3. Over repeated encounters, the B-being's warmth toward A-being crosses positive threshold
4. When enough cross-settlement positive impressions exist (>5 pairs with warmth > 0.3), the aggregate hostility between settlements declines
5. The viewer detects this: **"Relations warming between [A] and [B]"**

**Bridge-builder trait emergence:** beings who repeatedly share food across settlement lines accumulate causal memories of positive cross-settlement interactions. Their personality drifts slightly toward higher generosity (reinforcement). The player may notice specific beings who consistently mediate -- these are emergent diplomats, not programmed ones.

**Full peace:** when average warmth between settlement A and settlement B members crosses 0.0 (from negative to neutral), and no combat events between them for >2000 ticks, the viewer labels: **"Peace between [A] and [B]"**

### War Naming

The viewer detects sustained inter-settlement conflict and generates names:

**War detection criteria:**
- 5+ combat events between members of settlement A and settlement B within 3000 ticks
- At least 1 death on either side
- Average warmth between the groups < -0.3

**War name generation:**

```rust
fn generate_war_name(
    attacker: &Settlement,
    defender: &Settlement,
    deaths: u32,
    duration_ticks: u64,
) -> String {
    let scale = match deaths {
        0..=2 => "Skirmish",
        3..=7 => "Conflict",
        8..=15 => "War",
        16..=30 => "Great War",
        _ => "Devastation",
    };

    let cause = if avg_hunger(attacker) < 0.3 {
        "of Famine"
    } else if initial_trigger == CombatEvent::TakeFood {
        "of Greed"
    } else if initial_trigger == CombatEvent::Revenge {
        "of Vengeance"
    } else {
        "of Wrath"
    };

    // Examples: "The Skirmish of Famine", "The Great War of Vengeance"
    format!("The {} {}", scale, cause)
}
```

Wars are tracked in the World History log with: name, belligerents, start tick, end tick (when peace criteria met), total casualties, notable beings (most kills, bridge-builders who ended it).

---

## 9.2 God Powers -- Full Catalog

The Part 2 god tool palette defined a basic set. This section expands it to a full catalog of 68 powers organized into 8 tabs, replacing the simple Part 2 layout with the complete system. The left-panel layout remains (240px, collapsible), but each tab now scrolls independently.

### Tab 1: Creation (10 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 1 | Place Being | Humanoid silhouette, white | Spawn 1 being at click with preset personality. Drag to paint. Presets: Random, Warrior, Farmer, Explorer, Elder, Child. | Point | None (max 10/sec) |
| 2 | Place Deer Herd | Deer antler silhouette | Spawn 5 deer in cluster at click, random offsets within 4 units | 4-unit radius | None |
| 3 | Place Wolf Pack | Wolf head, gray | Spawn 3 wolves in pack at click, random offsets within 3 units | 3-unit radius | None |
| 4 | Place Bear | Bear paw print | Spawn 1 bear at click location | Point | None |
| 5 | Place Bird Flock | V-shape formation | Spawn 10 birds in flock at click, random offsets within 5 units | 5-unit radius | None |
| 6 | Place Fish School | Fish silhouette, blue | Spawn 8 fish in water cell nearest to click. Fails if no water within 10 units. | Water cell | None |
| 7 | Place Rabbit Warren | Rabbit ears | Spawn 6 rabbits near shelter cell closest to click | 3-unit radius | None |
| 8 | Drop Food | Apple, red | Deposit configurable food (slider 0.5-5.0) at click cell. Drag to paint. | 1 cell | None |
| 9 | Plant Berry Bush | Bush with berries, green/red | Set cell food_capacity=3.0, regrowth=0.003, type=Berries. Permanent. | 1 cell | None |
| 10 | Place Shelter | Lean-to structure, brown | Create shelter at click. Sets cell shelter=true, deposits comfort signal 0.5. Beings prioritize it for sleep and warmth. | 1 cell | None |

### Tab 2: Terrain (12 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 11 | Paint Forest | Tree, green | Set biome to Forest. Updates food_capacity=2.0, regrowth=0.001, movement_cost=1.5, shelter flags for edge cells. | Brush (1/3/5/10) | None |
| 12 | Paint Grassland | Grass blades, light green | Set biome to Grassland. food_capacity=1.2, regrowth=0.001, movement_cost=1.0. | Brush | None |
| 13 | Paint Desert | Sand dune, tan | Set biome to Desert. food_capacity=0.1, regrowth=0.0001, movement_cost=1.3. | Brush | None |
| 14 | Paint Mountain | Peak, gray | Set biome to Mountain. food_capacity=0.3, regrowth=0.0005, movement_cost=3.0. Impassable above height 0.9. | Brush | None |
| 15 | Paint Wetland | Cattails, dark green | Set biome to Wetland. food_capacity=0.8, regrowth=0.002, near_water=true, fish food type for adjacent. | Brush | None |
| 16 | Paint Water | Water drop, blue | Set cell to water. Impassable to land beings. Adjacent cells gain near_water, fish regrowth. | Brush | None |
| 17 | Raise Elevation | Up arrow on hill | Increase terrain height by 0.2 per click. Cycles: plain -> hill -> mountain -> summit. Movement cost increases with height. | Brush | None |
| 18 | Lower Elevation | Down arrow on valley | Decrease terrain height by 0.2 per click. Cycles: summit -> mountain -> hill -> plain -> shallow water -> deep water. | Brush | None |
| 19 | Create River | Wavy blue line | Click start point, click end point. Auto-generates winding water path between them using A* with random noise. Width: 1-2 cells. Adjacent cells gain wetland bonus. | Line (2 clicks) | None |
| 20 | Create Lake | Circle, blue fill | Click center. Creates circular water body, radius 3-8 cells (scroll to adjust). Smooth edges. Adjacent land becomes wetland. Fish auto-spawn (1 per 4 water cells). | Circle (3-8 radius) | None |
| 21 | Plant Trees | Scattered trees, green | Force-grow trees in area. Sets food_type to forest, adds canopy rendering, increases shelter value for cells under canopy. | Brush | None |
| 22 | Eraser | Pink eraser | Remove any terrain paint -- revert cells to default grassland biome with default parameters. | Brush | None |

### Tab 3: Weather (8 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 23 | Rain | Cloud with raindrops, blue | Trigger rain in area. 300 tick duration. Boosts food regrowth 2x, deposits comfort signal 0.1, extinguishes fire cells. | 20x20 cells | 600 ticks |
| 24 | Drought | Cracked earth, brown | Deplete food in area at 0.001/tick for 500 ticks. Regrowth halved. Beings in area get thirst (hunger decays 1.5x). | 20x20 cells | 1200 ticks |
| 25 | Storm | Lightning bolt in cloud, dark | Danger signal burst (1.0), warmth damage (-0.1 to all beings), scatter effect (beings flee center). 100 tick duration. Random lightning strikes (3-5) within area, each can kill 1 being if struck. | 15x15 cells | 900 ticks |
| 26 | Blizzard | Snowflake in wind, white | Warmth decay 5x for 400 ticks. Movement speed halved. Food regrowth stops. Beings without shelter lose warmth rapidly. Forces clustering/shelter-seeking behavior. | 25x25 cells | 1500 ticks |
| 27 | Heatwave | Sun with heat lines, orange | Warmth need satisfied (pinned to 1.0) for 300 ticks, but hunger decay 2x (dehydration). Water cells shrink (outer ring converts to wetland). Desert biome spreads 1 cell outward from existing desert. | 30x30 cells | 1200 ticks |
| 28 | Flood | Rising water, blue gradient | All cells in area become water for 1000 ticks, then revert to wetland. Beings pushed to edges. Food destroyed. After flood recedes, wetland biome = highly fertile. | 20x20 cells | 2000 ticks |
| 29 | Fog | Gray cloud, low | Reduces perception radius by 50% for all beings in area for 500 ticks. Beings cannot see danger signals beyond 4 units. Predators gain advantage (wolves hunt at 2x success rate). | 20x20 cells | 800 ticks |
| 30 | Aurora | Green/purple shimmer | Purely aesthetic + emotional: all beings in area get joy +0.2, belonging +0.1 for 200 ticks. Deposits celebration signal 0.3. Night only (if used during day, queues until next nightfall). | 40x40 cells | 3000 ticks |

### Tab 4: Destruction (10 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 31 | Lightning Strike | Jagged bolt, yellow | Instant kill on nearest being within 3 units of click. Triggers grief burst (2.0 strength), deposits danger signal (1.5). Spark particle + thunder sound. Witnesses within 10 units get fear +0.5. | Point (snap 3u) | 120 ticks |
| 32 | Earthquake | Cracked ground, brown | Destroys shelters in area (shelter flag set to false). Beings knocked down (stunned 30 ticks, no actions). Deposits danger signal (1.0). Terrain height randomized +/- 0.1. 5% chance per cell to create new shelter (cave). | 15x15 cells | 1800 ticks |
| 33 | Meteor | Flaming rock, orange/red | Impact at click point. All beings within 3 units: instant kill. Beings within 6 units: hunger -0.5, fear +0.8. Crater: 3-cell radius set to barren (food_capacity=0, regrowth=0). Deposits massive danger signal (2.0). Burns to fire for 200 ticks. | 6-unit radius | 3000 ticks |
| 34 | Plague | Green skull | Beings in area have all need decay rates doubled for 1500 ticks. Spreads: infected beings who touch uninfected beings within 2 units have 10% chance per tick to spread plague. Plague grid overlay (green tint on affected cells). | 10x10 cells | 2000 ticks |
| 35 | Famine | Wilted plant, brown | Set food to 0.0 in area, regrowth_rate to 0.0 for 2000 ticks, then restores. Berry bushes and forest food sources destroyed (must be replanted). | 15x15 cells | 2500 ticks |
| 36 | Wildfire | Flame spread, orange/red | Ignites center cell. Fire spreads to adjacent forest/grassland cells at 1 cell per 20 ticks. Burns for 100 ticks per cell, converting forest to barren, destroying food. Beings in burning cells take hunger damage (-0.05/tick) and flee. Stops at water, desert, mountain. | Spreading from point | 1500 ticks |
| 37 | Tornado | Spiral cone, gray | Moving column: travels in random direction at 0.1 units/tick for 300 ticks. Beings within 2 units are flung 10-20 units in random direction (teleported, take hunger damage -0.3 on landing). Destroys shelters in path. | 2-unit radius, moving | 2000 ticks |
| 38 | Sinkhole | Dark circle, descending | 5-cell radius area drops to water level. Beings in area teleported to edges. Permanent terrain change (creates a lake). Food and shelters in area destroyed. | 5-cell radius | 3000 ticks |
| 39 | Predator Swarm | Multiple red eyes | Spawns 8 wolves at click point, all with hunger=0.1 (starving, will attack immediately). Temporary: wolves have 50% normal lifespan. | Point | 2000 ticks |
| 40 | Extinction Pulse | Expanding red ring | All fauna (non-human beings) within radius die instantly. Humans unaffected. Useful for removing predator threat or testing human-only ecosystems. | 20-unit radius | 5000 ticks |

### Tab 5: Blessing (9 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 41 | Inspire Joy | Sun with smile, gold | All beings in area: `emotions[EMO_JOY] += 0.5`, clamped to 1.0. Deposits celebration signal (0.8). Golden particle burst. | 8x8 cells | 300 ticks |
| 42 | Inspire Courage | Shield with lion, gold | All beings in area: `emotions[EMO_FEAR] = (fear - 0.5).max(0.0)`, temporary bold boost +0.2 for 1000 ticks. Red/gold particle. | 8x8 cells | 500 ticks |
| 43 | Inspire Calm | Dove, white/blue | All beings in area: `emotions[EMO_ANGER] = 0.0`, `emotions[EMO_FEAR] = 0.0`, `emotions[EMO_CONTENTMENT] += 0.6`. Deposits comfort (1.0). Blue particle. | 8x8 cells | 400 ticks |
| 44 | Love Spark | Two hearts, pink | Select two beings (click, shift+click). Sets mutual warmth to 0.8, trust to 0.7. Instant bond. Heart particle between them. | 2 beings | 60 ticks |
| 45 | Heal | Cross, green/white | All beings in area: hunger restored to 1.0, warmth restored to 1.0. Cures plague. Green glow particle. | 6x6 cells | 600 ticks |
| 46 | Feast | Cornucopia, golden | Deposit 5.0 food at every cell in area. Equivalent to a massive food drop. Triggers celebration signal from nearby beings. | 10x10 cells | 1200 ticks |
| 47 | Shelter Gift | House with glow, warm | Create shelters at all valid cells in area (non-water, non-mountain-summit). Each shelter deposits comfort 0.5. Instant settlement infrastructure. | 8x8 cells | 1500 ticks |
| 48 | Longevity | Hourglass, gold | All beings in area: lifespan extended by 20% (multiply remaining ticks by 1.2). One-time. Stacks up to 3x (max 1.728x original lifespan). | 10x10 cells | 3000 ticks |
| 49 | Fertility | Sprout, green | All food cells in area: food_capacity doubled, regrowth_rate doubled for 2000 ticks. Berry bushes bloom instantly (food = capacity). Forest cells produce 2x food. | 15x15 cells | 2000 ticks |

### Tab 6: Curse (9 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 50 | Inspire Fear | Eye, red/black | All beings in area: `emotions[EMO_FEAR] += 0.7`. Bold beings resist partially (fear increase = `0.7 * (1.0 - bold * 0.5)`). Danger signal deposit (0.8). Dark particle. | 8x8 cells | 300 ticks |
| 51 | Inspire Anger | Fist, red | All beings in area: `emotions[EMO_ANGER] += 0.6`. Random anger targets assigned from nearby beings outside the area (seeds inter-settlement hostility). Red particle burst. | 8x8 cells | 400 ticks |
| 52 | Madness | Spiral, purple | All beings in area: personality traits randomized. Bold, social, curious, generous, diurnal all set to `rng.f32() * 2.0 - 1.0`. Beings lose coherent personality. Lasts 3000 ticks, then original personality restores (stored in temp buffer). Purple spiral particle. | 6x6 cells | 2000 ticks |
| 53 | Hunger Curse | Gnawing teeth, dark | All beings in area: hunger decay rate 3x for 1500 ticks. Beings become desperate -- TakeFood and Hunt action scores spike. Likely triggers raiding behavior. | 10x10 cells | 1500 ticks |
| 54 | Exile | Pointing hand, dark | Click a single being. Teleport them to nearest map edge (within 5 units of border). Reset all relationship impressions to 0. Being retains personality and memories but loses all social bonds. They must rebuild. | 1 being | 300 ticks |
| 55 | Distrust | Broken handshake, gray | All beings in area: trust toward all known beings reduced by 0.4. Warmth toward non-family reduced by 0.2. Fractures social bonds. Beings become suspicious, less likely to share food or cluster. | 10x10 cells | 1200 ticks |
| 56 | Amnesia | Erased brain, gray | All beings in area: causal memory cleared. Impressions toward all other beings reset to neutral (warmth=0, trust=0, debt=0). Beings forget who helped them and who hurt them. Relationships must be rebuilt from scratch. | 8x8 cells | 2500 ticks |
| 57 | Isolation | Walls closing in, dark | All beings in area: social trait temporarily set to -1.0 for 2000 ticks. Beings flee from all other beings (AvoidBeing scores maximum). Settlements dissolve as members scatter. | 10x10 cells | 2000 ticks |
| 58 | Mark of Hostility | Red X above being | Click a single being. All other beings within 15 units gain anger +0.3 toward the marked being. Trust toward them drops by 0.5. The marked being becomes a pariah -- attacked, shunned, driven out. Lasts 5000 ticks. | 1 being + 15u radius | 1500 ticks |

### Tab 7: Kingdom (10 powers)

These powers manipulate the viewer's settlement detection and the underlying relationship/impression data to simulate top-down political control.

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 59 | Force Alliance | Handshake, gold | Select two settlements (click, shift+click). Set warmth between all member pairs to +0.3 (minimum -- doesn't reduce existing higher warmth). Reset anger between groups to 0. Beings will now share food and cluster across settlement lines. | 2 settlements | 3000 ticks |
| 60 | Force War | Crossed swords, red | Select two settlements. Set anger toward all cross-settlement pairs to 0.5. Set warmth to -0.3 (minimum -- doesn't increase existing lower warmth). Armed bold beings immediately score TakeFood toward the other settlement. | 2 settlements | 3000 ticks |
| 61 | Crown Leader | Crown, gold | Click a being. That being's warmth toward all settlement members increased by +0.3. All settlement members' trust toward the crowned being increased by +0.4. The crowned being gets +0.2 bold, +0.2 generous (temporary, 10000 ticks). Viewer labels them as "[Settlement] Leader." | 1 being | 5000 ticks |
| 62 | Depose Leader | Broken crown, dark | Click a crowned being. Reverse Crown: warmth toward them drops by 0.5 in all settlement members. Trust drops by 0.6. Bold/generous boost removed. Being likely flees or is attacked if anger threshold crossed. | 1 being | 3000 ticks |
| 63 | Merge Settlements | Arrows converging, blue | Select two settlements within 30 units. All beings from both adopt the averaged warmth/trust values between them. Settlements are now one in the viewer's detection. Forces beings to share territory. | 2 settlements | 5000 ticks |
| 64 | Split Settlement | Axe splitting, red | Click a settlement. Beings are partitioned into two groups by k-means on position (2 clusters). Cross-group warmth reduced by 0.3. Each group forms its own settlement nucleus. | 1 settlement | 4000 ticks |
| 65 | Summon Migration | Compass arrow, green | Click a settlement, then click a destination. All settlement members get a temporary explore bias toward destination (1500 ticks). Equivalent to the settlement collectively deciding to relocate. Food and warmth needs still drive individual decisions, so weak/timid may not follow. | 1 settlement + dest | 3000 ticks |
| 66 | Inspire Trade | Coin exchange, gold | Select two settlements. Generous beings (generous > 0.2) from each settlement score ShareFood toward the other at 2x normal weight for 3000 ticks. Simulates trade routes -- beings carry food back and forth. Builds warmth between groups organically. | 2 settlements | 2000 ticks |
| 67 | Propaganda | Megaphone, red | Click a settlement. All members' anger toward the settlement's current lowest-warmth group increases by 0.3. Generous beings resist (increase halved). Seeds hostility toward perceived enemies. | 1 settlement | 1500 ticks |
| 68 | Revolution | Raised fist, red | Click a settlement's leader. All members with trust < 0.2 toward the leader gain anger +0.5 toward them. If enough beings become hostile (>40% of settlement), they attack the leader and potentially split the settlement. | 1 settlement | 5000 ticks |

### Tab 8: World (10 powers)

| # | Power | Icon | Effect | AoE | Cooldown |
|---|-------|------|--------|-----|----------|
| 69 | Toggle Seasons | Four-segment circle (spring/summer/autumn/winter colors) | Cycles through: Normal (seasons rotate) -> Locked to current season -> Manual (player selects). When locked, season multipliers stay constant. | Global | None (toggle) |
| 70 | Set Season | Leaf/snowflake/sun/flower depending on current | Force-set the current season. Only available when Toggle Seasons is set to Manual. Immediately applies season multipliers for selected season. | Global | None |
| 71 | Toggle Day/Night | Half circle (sun/moon) | Cycles: Normal (day/night rotate) -> Eternal Day -> Eternal Night. Eternal Day: diurnal beings always active, nocturnal penalty. Eternal Night: nocturnal beings active, diurnal beings forced to sleep more. | Global | None (toggle) |
| 72 | Set Time of Day | Clock face | Slider: 0.0 (midnight) to 1.0 (midnight). Snap points at 0.25 (dawn), 0.5 (noon), 0.75 (dusk). Jumps the day/night cycle to specified point. | Global | None |
| 73 | Fast-Forward 1 Year | Double arrow with "1Y" | Advance simulation by 36,000 ticks (60 game-days x 600 ticks/day). Runs headless (no rendering) for performance. Shows progress bar. Resumes rendering when complete. Warning: large populations may take several seconds. | Global | None (blocking) |
| 74 | Fast-Forward 1 Season | Arrow with season icon | Advance simulation by 9,000 ticks (15 game-days). Same headless execution as 1-year. | Global | None (blocking) |
| 75 | World Pause | Double bar, white | Freeze all simulation. Beings frozen in place. Player can still paint terrain, place beings, use tools -- changes queue and apply on unpause. Camera still navigable. | Global | None (toggle) |
| 76 | World Reset: Beings | Humanoid with refresh arrow | Kill all beings (human and fauna). Terrain, food, shelters untouched. Fresh start for population experiments on existing terrain. Confirmation dialog required. | Global | None (confirm) |
| 77 | World Reset: Terrain | Mountain with refresh arrow | Reset all terrain to default generation. Beings survive but are relocated to nearest valid cell. Food and shelters regenerated from biome defaults. Confirmation dialog. | Global | None (confirm) |
| 78 | Snapshot/Restore | Camera icon / Rewind icon | Save current world state (all being data + terrain + signals) to a snapshot slot (3 slots). Restore replaces current state with snapshot. File size: ~50MB for 10K beings on 256x256 grid. | Global | None |

**Total: 78 god powers across 8 tabs.**

### Engine Integration

All god powers operate through the `GodAction` event queue (defined in Part 2). New action variants:

```rust
enum GodAction {
    // Part 2 originals
    SpawnBeing { pos: [f32; 2], personality: [f32; 5], lifespan: u32 },
    DepositFood { x: u32, y: u32, amount: f32 },
    SetBiome { x: u32, y: u32, biome: Biome },
    TriggerWeather { kind: WeatherKind, region: Rect, duration: u32 },
    KillBeing { index: usize },
    FloodArea { region: Rect, duration: u32 },
    InspireArea { region: Rect, emotion: usize, intensity: f32 },
    LoveSpark { a: usize, b: usize },

    // Part 9 additions
    SpawnFauna { kind: CreatureType, pos: [f32; 2], count: u8 },
    SetElevation { x: u32, y: u32, delta: f32 },
    CreateRiver { start: (u32, u32), end: (u32, u32) },
    CreateLake { center: (u32, u32), radius: u8 },
    PlagueCast { region: Rect, duration: u32 },
    WildfireIgnite { x: u32, y: u32 },
    Tornado { pos: [f32; 2], duration: u32 },
    ModifyEmotions { region: Rect, changes: [(usize, f32); 6] },
    ModifyImpressions { a_group: Vec<usize>, b_group: Vec<usize>, warmth: f32, trust: f32, anger: f32 },
    ModifyPersonality { indices: Vec<usize>, trait_idx: usize, delta: f32, duration: u32 },
    ClearMemory { indices: Vec<usize> },
    TeleportBeing { index: usize, target: [f32; 2] },
    SetSeason { season: Season },
    SetDayNightMode { mode: DayNightMode },
    FastForward { ticks: u64 },
    WorldReset { kind: ResetKind },
    Snapshot { slot: u8 },
    Restore { slot: u8 },
    ModifyNeeds { indices: Vec<usize>, changes: [(usize, f32); 6] },
    SetFoodCapacity { region: Rect, capacity: f32, regrowth: f32, duration: u32 },
    SpawnShelter { x: u32, y: u32 },
    ExtendLifespan { indices: Vec<usize>, multiplier: f32 },
    MarkHostile { target: usize, radius: f32, anger: f32, duration: u32 },
}
```

All actions are processed at tick start, before climate/resource/signal updates. This prevents mid-tick state corruption.

---

## 9.3 World Laws -- Simulation Parameter Toggles

World Laws are toggleable parameters that the player can flip anytime via the World Laws panel (accessible from top bar or keyboard shortcut `L`). Each law maps to a specific engine parameter override. Laws take effect immediately on toggle -- no delay, no transition.

### Law Panel UI

```
+------------------------------------------+
| WORLD LAWS                          [X]  |
|                                          |
| SURVIVAL                                 |
|   [ON ] Hunger Enabled                   |
|   [ON ] Warmth Enabled                   |
|   [ON ] Aging Enabled                    |
|   [OFF] Immortal Beings                  |
|   [OFF] No Sleep Required                |
|                                          |
| POPULATION                               |
|   [ON ] Reproduction Enabled             |
|   [ON ] Natural Death (old age)          |
|   [OFF] Population Cap (slider: 500)     |
|   [OFF] Fast Growth (2x reproduction)    |
|                                          |
| BEHAVIOR                                 |
|   [ON ] Combat Enabled                   |
|   [OFF] Peaceful Mode                    |
|   [ON ] Raiding Enabled                  |
|   [ON ] Fear Enabled                     |
|   [ON ] Anger Enabled                    |
|   [OFF] Max Generosity (all share)       |
|   [ON ] Personality Drift                |
|                                          |
| LEARNING                                 |
|   [ON ] Causal Memory                    |
|   [ON ] Witnessing                       |
|   [OFF] Fast Learning (2x memory weight) |
|   [OFF] Perfect Memory (no forgetting)   |
|                                          |
| ECOLOGY                                  |
|   [ON ] Fauna Enabled                    |
|   [ON ] Predators Hunt Beings            |
|   [ON ] Food Regrowth                    |
|   [OFF] Infinite Food                    |
|   [ON ] Seasonal Effects                 |
|                                          |
| TIME                                     |
|   [ON ] Day/Night Cycle                  |
|   [ON ] Seasons                          |
|   [OFF] Slow Aging (0.5x)               |
|   [OFF] Fast Aging (3x)                  |
|                                          |
+------------------------------------------+
```

### Full Law Catalog (28 laws)

#### Survival Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 1 | Hunger Enabled | ON | `HUNGER_DECAY_RATE = 0.0004` vs `0.0` | OFF: hunger pinned to 1.0. Beings never need food. Removes foraging/sharing/raiding pressure entirely. |
| 2 | Warmth Enabled | ON | `WARMTH_DECAY_RATE = 0.001` vs `0.0` | OFF: warmth pinned to 1.0. Beings never seek shelter or cluster for warmth. Winter has no teeth. |
| 3 | Aging Enabled | ON | `age_increment = 1` vs `0` per tick | OFF: beings never age past their current age. Youth stays youth. Adults stay adult. No elder stage, no natural death from old age. Combined with Immortal = true stasis. |
| 4 | Immortal Beings | OFF | `starvation_death = false, age_death = false, combat_death = false` | ON: beings cannot die from any cause. Hunger/warmth still cause behavioral distress but never kill. Population only grows. Useful for observing social dynamics without death noise. |
| 5 | No Sleep Required | OFF | `REST_DECAY_RATE = 0.0`, `needs[REST] = 1.0` | ON: beings never tire. They act 24/7. Removes day/night behavioral variation. Nocturnal/diurnal personality trait becomes meaningless. |

#### Population Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 6 | Reproduction Enabled | ON | `reproduction_enabled = true/false` | OFF: no new births. Population can only shrink. Useful for controlled experiments. |
| 7 | Natural Death | ON | `age_death = true/false` | OFF: beings don't die from old age (but can still die from starvation/combat). Different from Immortal -- they're still vulnerable, just not to time. |
| 8 | Population Cap | OFF | When ON, `max_population = slider_value (100-10000)`. If `alive_count >= max`, reproduction disabled and new god-placed beings fail. | Prevents runaway population. Slider default: 500. Useful for performance or focused experiments. |
| 9 | Fast Growth | OFF | `reproduction_chance *= 2.0`, `youth_duration *= 0.5` | ON: beings reproduce twice as often and youth mature in half the time. Rapid population growth for testing. |

#### Behavior Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 10 | Combat Enabled | ON | `combat_resolution_enabled = true/false` | OFF: TakeFood action never results in combat damage. Beings can still approach aggressively but cannot hurt each other. Anger still builds, but cannot be expressed violently. |
| 11 | Peaceful Mode | OFF | Sets: combat_enabled=false, anger_enabled=false, fear_enabled=false, raiding_enabled=false | ON: master switch for all conflict. Beings are calm, cooperative, and fearless. Pure social/economic simulation. |
| 12 | Raiding Enabled | ON | `raiding_detection = true/false` and `cross_settlement_TakeFood_scoring = normal/zero` | OFF: beings never score TakeFood toward beings from other settlements. They can still take food from the ground, just not from other beings they're hostile toward. Removes inter-group conflict. |
| 13 | Fear Enabled | ON | `EMO_FEAR` decay/accumulation active vs pinned to 0.0 | OFF: beings feel no fear. They never flee. Bold or timid, they stand their ground. Wolves approach and beings don't run. Interesting for observing courage without the retreat option. |
| 14 | Anger Enabled | ON | `EMO_ANGER` decay/accumulation active vs pinned to 0.0 | OFF: beings feel no anger. No grudges, no revenge, no hostility. Combined with Fear OFF = beings are emotionally flat (only joy, contentment, grief remain). |
| 15 | Max Generosity | OFF | `personality[GENEROUS]` pinned to 1.0 for all beings | ON: every being acts maximally generous. All food is shared. No hoarding. Communist utopia experiment. Interesting to observe: does unlimited generosity lead to sustainability or resource collapse? |
| 16 | Personality Drift | ON | `personality_drift_enabled = true/false` | OFF: personality traits fixed at birth values. Experience does not change who beings are. Nature without nurture. |

#### Learning Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 17 | Causal Memory | ON | `causal_memory_enabled = true/false` | OFF: beings do not form new causal memories from events. They still have impressions (warmth/trust from direct interaction) but don't remember WHY they feel a certain way. Reduces memory overhead significantly. |
| 18 | Witnessing | ON | `witnessing_enabled = true/false` | OFF: beings only learn from direct interaction, not from observing others. Reputation doesn't spread. A thief is only known to their victims, not bystanders. Fundamentally changes social dynamics. |
| 19 | Fast Learning | OFF | `memory_weight_multiplier = 2.0` | ON: impressions change at 2x rate. One positive interaction has the impact of two. Beings form opinions faster, for better or worse. Grudges set in quicker, friendships form faster. |
| 20 | Perfect Memory | OFF | `memory_decay_rate = 0.0` | ON: impressions never decay toward neutral. Every slight, every kindness, every betrayal is remembered at full strength forever. Creates deeply committed relationships and bitter, eternal grudges. |

#### Ecology Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 21 | Fauna Enabled | ON | `fauna_spawn = true/false`, `fauna_update = true/false` | OFF: no animals in the world. Removes all fauna beings from simulation. Humans only. Saves ~1500 being slots and ~1.5ms/tick. |
| 22 | Predators Hunt Beings | ON | `predator_targets_human = true/false` | OFF: wolves and bears never target human beings. They hunt deer/rabbits only. Removes wildlife danger for a gentler simulation. |
| 23 | Food Regrowth | ON | `regrowth_rate = normal` vs `0.0` | OFF: food cells never regenerate. Once eaten, gone forever. The world is a finite resource. Creates mounting desperation. How long until beings turn on each other? |
| 24 | Infinite Food | OFF | `food = capacity` forced every tick (food never depletes) | ON: every food cell is always full. Hunger is trivially satisfied. Removes all food-driven behavior (foraging, sharing, raiding, hunting). Beings focus purely on social/safety/belonging needs. |
| 25 | Seasonal Effects | ON | `season_multiplier = seasonal_value` vs `1.0` always | OFF: no seasonal variation. Food regrowth constant. Temperature constant. No migration, no winter pressure, no spring boom. Flattens the ecological cycle. |

#### Time Laws

| # | Law | Default | Engine Parameter Override | Effect |
|---|-----|---------|--------------------------|--------|
| 26 | Day/Night Cycle | ON | `day_night_enabled = true/false` | OFF: permanent daylight. Diurnal/nocturnal personality irrelevant. No nighttime fear bonus. No wolf nighttime hunting bonus. |
| 27 | Slow Aging | OFF | `age_increment = 0.5` per tick (vs normal 1.0) | ON: beings age at half speed. Lifespans effectively double. More time for relationships to develop. Generations overlap longer. Cannot combine with Fast Aging. |
| 28 | Fast Aging | OFF | `age_increment = 3.0` per tick | ON: beings age at 3x. Generations turn over rapidly. Civilizations rise and fall in minutes of real time. Good for long-arc observation. Cannot combine with Slow Aging. |

### Law Implementation

Laws are stored as a bitfield + parameter overrides in the World struct:

```rust
struct WorldLaws {
    flags: u32,                    // 28 bits, one per law
    population_cap: u32,           // only used when POPULATION_CAP flag set
    aging_speed: f32,              // 0.5, 1.0, or 3.0
}

impl WorldLaws {
    const HUNGER_ENABLED: u32      = 1 << 0;
    const WARMTH_ENABLED: u32      = 1 << 1;
    const AGING_ENABLED: u32       = 1 << 2;
    const IMMORTAL: u32            = 1 << 3;
    const NO_SLEEP: u32            = 1 << 4;
    const REPRODUCTION: u32        = 1 << 5;
    const NATURAL_DEATH: u32       = 1 << 6;
    const POPULATION_CAP: u32      = 1 << 7;
    const FAST_GROWTH: u32         = 1 << 8;
    const COMBAT: u32              = 1 << 9;
    const PEACEFUL: u32            = 1 << 10;
    const RAIDING: u32             = 1 << 11;
    const FEAR: u32                = 1 << 12;
    const ANGER: u32               = 1 << 13;
    const MAX_GENEROSITY: u32      = 1 << 14;
    const PERSONALITY_DRIFT: u32   = 1 << 15;
    const CAUSAL_MEMORY: u32       = 1 << 16;
    const WITNESSING: u32          = 1 << 17;
    const FAST_LEARNING: u32       = 1 << 18;
    const PERFECT_MEMORY: u32      = 1 << 19;
    const FAUNA: u32               = 1 << 20;
    const PREDATORS_HUNT: u32      = 1 << 21;
    const FOOD_REGROWTH: u32       = 1 << 22;
    const INFINITE_FOOD: u32       = 1 << 23;
    const SEASONAL_EFFECTS: u32    = 1 << 24;
    const DAY_NIGHT: u32           = 1 << 25;
    const SLOW_AGING: u32          = 1 << 26;
    const FAST_AGING: u32          = 1 << 27;

    fn is_enabled(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}
```

**Law checks are inlined at the relevant engine points.** For example:

```rust
// In decay_needs():
if world.laws.is_enabled(WorldLaws::HUNGER_ENABLED) {
    beings.needs[i][NEED_HUNGER] = (beings.needs[i][NEED_HUNGER] - HUNGER_DECAY_RATE).max(0.0);
} else {
    beings.needs[i][NEED_HUNGER] = 1.0;
}
```

Cost: one branch per being per need per tick. Predicted correctly 99.9% of the time (law doesn't change mid-tick). Zero measurable overhead.

### Law Interaction Rules

Some laws conflict. The UI enforces:

| If you enable... | Auto-disables... |
|-----------------|------------------|
| Immortal Beings | (nothing -- immortal is additive) |
| Peaceful Mode | (force-sets: Combat OFF, Anger OFF, Fear OFF, Raiding OFF) |
| Infinite Food | Food Regrowth (redundant) |
| Slow Aging | Fast Aging |
| Fast Aging | Slow Aging |
| No Sleep Required | (nothing) |
| Population Cap | (nothing -- works alongside reproduction) |

### Interesting Law Combinations for Experimentation

| Combo | What Happens |
|-------|-------------|
| Immortal + No Reproduction | Fixed population forever. Pure social dynamics, no death noise. Watch relationship networks crystallize over thousands of years. |
| Infinite Food + Anger Enabled + Combat Enabled | Beings don't fight over food but still have personality conflicts. Pure social warfare. |
| No Fear + Predators Hunt | Beings stand their ground against wolves. Bold and timid react the same. Tests courage without the retreat option. |
| Perfect Memory + Fast Learning | Hyper-consequential world. One betrayal = permanent enemy. One kindness = permanent ally. Social graph becomes rigid fast. |
| No Food Regrowth + Fast Aging | Apocalypse mode. Resources deplete, beings age fast, civilization collapses. How long does cooperation last? |
| Peaceful + No Hunger + No Warmth | Pure belonging/purpose simulation. Beings focus entirely on social bonds and meaning. The emotional core of the engine, isolated. |
| Max Generosity + No Food Regrowth | Tragedy of the commons. Everyone shares everything, but the pie is shrinking. When does utopia break? |
| Fast Growth + Population Cap (100) | Rapid turnover within a fixed population. Generational change visible in minutes. |

---

## 9.4 Implementation Phase

Part 9 systems are **Phase 8** in the implementation priority, after the living ecosystem:

### Phase 8: Warfare, God Powers & World Laws

1. **World Laws struct** -- add to World, implement flag checks at all engine points (needs decay, combat resolution, memory, aging, reproduction). This is foundational -- everything else depends on laws working.
2. **Combat resolution system** -- implement `resolve_combat()` with hit chance, damage, witnessing, and causal memory updates.
3. **Raid detection in viewer** -- group hostile movements, label raids, track wars in event log.
4. **War naming** -- procedural war name generation from conflict metadata.
5. **Peace detection** -- track warmth recovery between hostile groups, label peace events.
6. **Expanded god tool palette** -- implement all 78 powers as `GodAction` variants, wire to UI.
7. **Tab 2 terrain tools** -- river/lake generation, elevation manipulation.
8. **Tab 3 weather tools** -- blizzard, heatwave, fog, aurora (new weather types beyond Part 2 basics).
9. **Tab 4 destruction tools** -- wildfire spread, tornado movement, sinkhole terrain modification.
10. **Tab 5-6 blessing/curse tools** -- emotion/personality/memory modification powers.
11. **Tab 7 kingdom tools** -- impression bulk modification, settlement detection manipulation.
12. **Tab 8 world tools** -- season/time overrides, fast-forward, snapshot/restore.
13. **World Laws UI panel** -- toggle switches, slider for population cap, mutual exclusion enforcement.
14. **Siege detection** -- viewer tracks prolonged occupation, population displacement.
15. **Bridge-builder detection** -- viewer identifies beings who consistently share food across hostile lines.

---

## Performance Impact

| Component | Cost |
|-----------|------|
| Combat resolution (per combat pair per tick) | ~0.5 microseconds |
| Raid/war/peace detection (viewer, per 600 ticks) | ~2ms per check |
| World Laws flag checks (per being per tick) | ~0.01 microseconds (branch prediction eliminates cost) |
| God power processing (per action queued) | 0.1-50ms depending on action (most < 1ms, fast-forward is blocking) |
| War naming / event log | Negligible (string generation on detection only) |
| Snapshot save (10K beings, 256x256 grid) | ~200ms (blocking, runs on keypress) |
| Snapshot restore | ~150ms (blocking) |

**No steady-state performance regression.** Combat resolution only fires when beings are adjacent and hostile (rare per tick -- maybe 5-20 combats per tick during active wars out of 10K beings). Law checks are branch-predicted into oblivion. Detection runs every 600 ticks in the viewer thread, not the engine thread.
