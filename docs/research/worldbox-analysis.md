# WorldBox Feature Analysis - Comprehensive Breakdown

> Research compiled for swarm-os architecture matching. Target: 99.99% WorldBox parity + AI-native differentiators.

---

## 1. VISUAL SYSTEM

### 1.1 Art Style
- **Pixel art**, top-down 2D perspective
- Built on **Unity engine** (confirmed via save paths and file formats)
- Tile-based rendering: each world tile is a single pixel-unit
- Creatures are small animated sprites (~8-16px depending on zoom), rendered as 2D sprites on a tile grid
- Distinctive chunky, colorful aesthetic -- retro but modern palette richness

### 1.2 Creature Visual Design
- **118 distinct creature types** as of version 0.51
- **4 civilized races**: Humans, Elves, Dwarves, Orcs -- each with unique sprite sets for:
  - Male/female citizens
  - Warriors
  - Kings/leaders
  - Children
- **Race-specific building styles**: Each race has unique house, town hall, windmill, and watchtower sprites across multiple tiers
- Creatures have visual variation via:
  - Gender (male/female sprites)
  - Skin sets (configurable per culture)
  - Equipment rendering (weapons, helmets, armor visible on sprite)
  - Trait-based visual modifiers (Giant = +50% size, Tiny = -20% size, Fat = +20% size)
  - Status effects (burning particles, frozen overlay, blessed glow, zombie green tint)
  - Age (Wise trait turns hair gray)
  - Madness changes clothes to red color (#E53B3B)

### 1.3 Creature Categories (118 total)
| Category | Examples | Count |
|----------|---------|-------|
| Civilized Races | Humans, Elves, Dwarves, Orcs | 4 |
| Domestic Animals | Cat, Dog, Chicken, Sheep, Cow, Rabbit | ~15 |
| Wild Animals | Wolf, Bear, Rhino, Buffalo, Hyena, Crocodile, Snake | ~20 |
| Insects | Bee, Butterfly, Beetle, Grasshopper, Fly | ~5 |
| Langton Ants | Blue, Green, Black, Red Ant | 4 |
| Marine | Piranha, Crab, Seal, Frog | ~5 |
| Monsters | Dragon, Demon, Zombie, Skeleton, Evil Mage, White Mage, Necromancer, Cold One | ~15 |
| Evolved/Mutant | Meowmorph, Barkfolk, Pecklord, Hopper, Monke, Slypaw, etc. | ~20 |
| Plant Creatures | Flower Bud, Crystal Sword, Bitba, Garl, Smore, Lulliar, Crystal Golem | ~10 |
| Special | Crabzilla, Grey Goo, UFO, God Finger, Assimilator, Greg | ~10 |
| Humanoid Mobs | Bandit, Snowman, Plague Doctor, Druid, Alien | ~10 |

### 1.4 Zoom Levels
- Smooth continuous zoom from world overview to close-up
- **Zoomed out**: creatures shown as colored dots or small icons; kingdom borders, army positions, and battle indicators visible via overlay toggles
- **Mid zoom**: sprites visible with basic animation, buildings distinguishable
- **Zoomed in**: full sprite detail, equipment visible, speech/thought bubbles shown, name labels available
- **Minimap**: shows biome colors with optional species icons, kingdom borders, army positions
- Toggle overlays: Kingdom Layer, Village Layer, Clan Layer, Religion Layer, Culture Layer, Language Layer, Family Layer, Subspecies Layer

### 1.5 Visual Effects
| Effect | Description |
|--------|-------------|
| Fire | Spreading flame particles on terrain and structures |
| Lightning | Strike from sky, sets wood on fire, heats pixels |
| Tornado | Rotating column of air, picks up creatures |
| Rain | Water droplets, extinguishes fire, global in Age of Tears |
| Snow | Particle overlay during Ice/Despair ages |
| Acid | Green dissolving particles eating terrain |
| Lava | Glowing orange/red flow, cools to rock |
| Explosions | TNT, bombs, nukes -- crater formation + shockwave |
| Zombie infection | Green particle effect on attack and passively |
| Blessing glow | Golden/white particle effect |
| Burning status | Fire particles attached to creature |
| Frozen status | Ice overlay on creature |
| Death | Creature disappears, may leave corpse/items |
| Combat | Hit particles, knockback animations |
| Lightning on death | Energized trait spawns small lightning |
| Fire on death | Fire Blood trait spawns flames |
| Acid on death | Acid Blood spawns acid particles |
| Shiny | Sparkle visual effect (Crystal creatures) |
| Flower Prints | Flowers grow where creature walks |
| Burning Feet | Ground burns where creature walks |
| Cold Aura | Ground freezes around creature |
| Healing Aura | Soft glow healing nearby creatures |
| Speech Bubbles | Thought/speech bubbles above creatures |
| Money Flow | Animated coins during trades/taxation |
| Age Overlays | Darkness, Moon glow, Chaos red, Magic sparkles, Rain, Ash particles, Snow, Sun heat |

### 1.6 Buildings & Structures
- **Race-specific architecture**: Each of the 4 races has unique building sprites
- Building types:
  - **Town Hall** (3 tiers): Bonfire -> Hall -> Castle
  - **Houses** (6 tiers): progressively larger/more detailed
  - **Windmill** (2 tiers): enables farm creation
  - **Mine** (1 per village): extracts stone, iron, gold, mythril, adamantine
  - **Watch Towers**: border defense, count as +10 occupying force
  - **Docks** (up to 5): built near water for trade boats
  - **Barracks** (1 per village): military training
  - **Statue** (1 per village): cultural building
  - **Well** (1 per village): water source
  - **Roads**: Dwarves and Humans build roads connecting buildings
  - **Farms**: Circular area (radius 9 tiles) around windmill with field tiles
- Construction: 1 building at a time per city, 1-month planning cooldown
- **Dwarven cities** are tidier (20% center tile, 80% adjacent to center for building placement)
- Other races place buildings randomly within zones

### 1.7 Nature Rendering
- **31 biome types**: Grassland, Savanna, Birch, Maple, Swamp, Jungle, Corrupted, Infernal, Mushroom, Candy, Garlic, Lemon, Enchanted, Arcane Desert, Rocklands, Crystal, Permafrost, Flower Meadow, Celestial, Singularity Swamp, Paradox, Clover, plus terrain types
- Each biome has unique:
  - Ground texture/color
  - Tree sprites (multiple per biome)
  - Plant/flower sprites
  - Characteristic fauna
- **Resources** rendered as tile objects: Stone, Iron (Ore Deposit), Silver, Mythril, Adamantine, Gold
- **Terrain heights**: Deep Ocean, Close Ocean, Shallow Waters, Sand, Plain Soil, Forest Soil, Hills, Mountains, Summit
- **7 wall types**: Stone, Evil, Ancient, Wooded, Green, Iron, Light

---

## 2. GOD TOOLS (Core Gameplay)

### 2.1 Power Tab Organization (~374 powers across 8 tabs)

#### Tab 1: Main (15 powers)
- Pause/Resume, Hourglass (time speed)
- World Statistics, Eye of Insight (trait discovery)
- World Laws, Ages, World History
- Game Statistics, Create New World, Your Worlds
- Game Settings, Achievements, Community
- Hide UI, Steam Workshop

#### Tab 2: Unit (8 powers, appears when unit selected)
- Main Info, Possession (take control!)
- Add to Favorites, Follow on Map
- Trait Editor, Equipment Editor
- Mind (inner mind networks), Genealogy (family tree)

#### Tab 3: World Shaping (47 powers)
- **Terrain painting**: 9 terrain types (Deep Ocean through Summit)
- **7 wall types**: Stone, Evil, Ancient, Wooded, Green, Iron, Light
- **Terrain tools**: Shovel Up/Down, Finger (copy), Vortex (distort), Sponge (clean)
- **Removal tools**: Sickle (grass), Bucket (water/lava), Pickaxe (resources), Spade (biome), Axe (trees), Demolish (buildings), Life Eraser, Scissors (city zones)
- **City tools**: Paint (expand borders), Border Brush
- **Fun tools**: Fuse, Fireworks
- **Printers**: 13 shape stamps (Hexagon, Skull, Squares, Yinyang, Island, Star, Heart, Diamond, Alien, Crater, Labyrinth, Spiral, Star Fort, Code)

#### Tab 4: Noosphere and Life (61 powers)
- **Civilization management**: Village Info, Relations, Compare Statistics
- **Diplomatic tools**: Spite (force war), Friendship (force peace), Inspiration (create kingdom), Whisper of War, Discord (break alliances), Unity (form alliances)
- **Favorites system**: Favorited Creatures, Items, and their map icons
- **Overlay toggles**: Wars, Armies, Alliances, Kingdoms, Villages, Clans, Religions, Cultures, Languages, Families, Subspecies -- each with zone layer toggle
- **Map display**: Zone overlays, Species Icons, Map Names, Kings/Leaders, Boats, Show Armies, Show Battles
- **Info overlays**: Highlight Kingdom Enemies, Army Targets, Important Events, Meta Pins, Happiness Icons, Highlighted Favorites, Task Icons, Money Flow, Bubbles, Conversations
- **Tooltips**: Zone tooltips, Unit tooltips (Steam only)
- **Arrow overlays**: Destination, Lover, House, Family, Parents, Kids, Attack Target (Steam only)

#### Tab 5: Animals, Creatures and Monsters (117 powers)
- Spawn any of the 118 creature types directly onto the map
- Includes civilized races, animals, monsters, evolved creatures, plant creatures, insects, special entities

#### Tab 6: Nature and Disasters (53 powers)
- **Weather**: Temperature Hot/Cold, Lightning, Earthquake, Tornado, Rain, Fire, Acid, Lava
- **Biome seeds**: 31 biome types plantable
- **Fertilizers**: Plants Fertilizer, Trees Fertilizer
- **Resources**: Fruit Bush, Stone, Ore Deposit, Silver, Mythril, Adamantine, Gold
- **Geysers**: Acid Geyser, Geyser, Volcano
- **Clouds** (9 types): Cloud of Life, Rain Cloud, Thunder Cloud, Hell Cloud, Acid Cloud, Lava Cloud, Ash Cloud, Magic Cloud, Snow Cloud, Rage Cloud

#### Tab 7: Destruction and Chaos (37 powers)
- **Physics**: Force (bounce), Flick (yeet)
- **Explosives**: TNT, Delayed TNT, Water Bomb, Landmine, Grenade, Bomb, Napalm Bomb, Atomic Bomb, Antimatter Bomb, Tsar Bomba
- **Projectiles**: Bowling Ball, Meteorite
- **Weapons**: Heat-Ray, Infinity Coin (Thanos snap)
- **Special entities**: Robot Santa, Grey Goo, Crabzilla
- **Infections**: Zombie, MUSH Spores, The Plague, Madness, Corrupted Brain
- **Desire artifacts**: Golden Egg, Ethereal Harp, Alien Mold, Computer Chip + their desire powers
- **Animation**: Living Plants, Living Houses
- **Conway**: Game of Life (Pink), Game of Life (Green)

#### Tab 8: Other Various Powers (31 powers)
- **Creature buffs/debuffs**: Divine Magnet, Divine Light (heal), Blood Rain, Shield, Blessing, Curse, Coffee (speed), Powerup, Clone Rain, Smooth Jazz (breeding), Dispel, Sleep
- **Memory wipe dusts**: Black (forget everything), White (forget language), Red (forget family), Gold (forget kingdom), Blue (forget culture), Purple (forget religion)
- **Trait rains**: Gamma Rain (good traits), Omega Rain (bad traits), Delta Rain (weird traits), Loot Rain (weapons)
- **Meta**: About WorldBox, Tutorial Bear, News, Monolith (evolution), Golden Brain

### 2.2 Brush System
- Variable brush sizes for terrain painting and creature placement
- Printers for stamping predefined shapes
- Border Brush for drawing kingdom borders
- Paint tool for expanding city zones

### 2.3 Favorites/Toolbar
- Favorite individual creatures (star above unit, tracked in favorites list)
- Favorite items
- Map icons toggled for favorites
- Highlighted favorites on map

---

## 3. GAMEPLAY LOOP

### 3.1 Core Loop
1. **Create world** (choose map type, size, settings)
2. **Shape terrain** (paint land, mountains, oceans, biomes)
3. **Seed life** (place races, animals, resources)
4. **Observe emergence** (civilizations form, build, expand, war)
5. **Intervene as god** (help, hinder, destroy, experiment)
6. **Repeat/experiment** (new scenarios, what-if experiments)

### 3.2 What Keeps Players Engaged
- **Emergent storytelling**: Every world generates unique narratives
- **Experimentation freedom**: No win/lose conditions, pure sandbox
- **Achievement hunting**: Unlocking traits, creatures, powers through gameplay discovery
- **Community sharing**: Steam Workshop maps, Discord community
- **Frequent updates**: Active developer (Maxim Karpenko) adds new content regularly
- **Idle observation**: Watch civilizations evolve with minimal intervention
- **Destruction catharsis**: Unleash disasters after building up civilizations

### 3.3 Civilization Emergence
1. Place a civilized race on suitable terrain
2. They automatically:
   - Found a village (build bonfire)
   - Build town hall, then houses
   - Collect resources (wood, stone, ore)
   - Build mine, windmill, farms
   - Upgrade buildings through resource tiers
   - Expand borders, claim new zones
   - Send settlers to found new villages
   - Form kingdoms with kings, clans, cultures
   - Develop armies, watchtowers
   - Engage in diplomacy, trade, war

### 3.4 War & Conflict System
- **War types**: Conquest (natural), Spite (forced), Whisper (forced), Rebellion (natural), Inspired (forced)
- **Armies**: Organized military units with warriors, leaders
- **Village capture**: Progressive occupation system -- occupying force (watchtowers = +10, soldiers = +1 each), capture progress updates every 0.02 months
- **Capture at 5%**: Stalls if defending forces remain; all defenders must be eliminated to progress beyond 5%
- **Village destruction**: When population reaches 0, borders decay, village destroyed
- **Peace talks**: Kingdoms can negotiate peace (diplomatic plot)
- **War names**: Procedurally generated based on war type (e.g., "Kingdom's Great Conquest of Steel")

### 3.5 Kingdom System
- **Components**: Name, Banner (texture + symbol + color), Motto, King, Villages, Traits, Stats
- **Village loyalty**: Affected by distance, kingdom max villages, king stats
  - Village limit varies by race: Humans +5, Orcs +4, Elves +3, Dwarves +3
  - King bonuses: +1 per 6 stewardship, traits affect limit
  - Over-limit: -25 loyalty per excess village
- **Rebellions**: Triggered when loyalty < 0, requires warrior count > kingdom internal power
- **Clans**: Royal bloodlines, led by chief, with their own banners and levels
- **Alliances**: Multi-kingdom alliances with shared banner
- **Diplomacy plots**: War, peace, alliance, rebellion -- all driven by opinion system

### 3.6 Ages / Era System (10 Ages)
| Age | Duration | Key Effects |
|-----|----------|-------------|
| Hope | 35-55 years | Default, bright, +15 loyalty/opinion |
| Sun | 35-55 years | Drought, kills trees, fire spread x2, +15 temp |
| Dark | 35-55 years | 50% less weapon range, 50% crop death |
| Tears | 35-55 years | Global rain, spawns thunder, extinguishes fire |
| Moon | 35-55 years | Moonchild/Nightchild trait activation |
| Chaos | 35-55 years | -55 loyalty, -35 opinion, rage turns to demons |
| Wonders | 35-55 years | Magic clouds, enchantment |
| Ice | 30-40 years | Freezes world, kills crops, damages hydrophobic |
| Ash | 35-55 years | Sickness, ash fever, -25 loyalty |
| Despair | 30-40 years | Freeze + dark, children become Cold Ones |

- Ages rotate on an **Age Clock** with 8 slots
- Hope occupies ~53% of game time by default
- Player can toggle/control age flow

### 3.7 Technology Progression
- **No explicit tech tree** -- progression is resource-driven:
  1. Wood + Stone -> Mine, basic buildings
  2. Stone + Iron -> Upgraded town hall
  3. Iron + Gold -> Top-tier buildings and equipment
  4. Equipment crafted at 3 iron per item
- Building tier upgrades serve as visual tech progression
- Culture researches tech IDs (stored in save data)
- No modern/industrial/space age -- stays medieval fantasy

### 3.8 Trait System (116 creature traits + 204 subspecies traits)
- **Categories**: Cognitive, Mind, Spirit, Physique, Health, Body, Appearance, Protection, Skills, Merits, Acquired, Fun, Fate, Miscellaneous, Special
- **Rarities**: Normal, Rare, Epic, Legendary (achievement-locked)
- **Acquisition**: Random at birth, from food, from biomes, from combat, from aging, from player editing
- **Notable traits**: Immortal, Genius, Evil, Blessed, Chosen One, Giant, Tiny, Fast, Strong, Regeneration, Fire Blood, Acid Blood, Zombie, Plague, etc.
- **Subspecies traits**: 204 traits for genetic modification of entire subspecies
- **Gene Editor**: Player can modify traits and genes

### 3.9 World Laws (46+ toggleable rules)
Organized into categories:
- **Harmony**: Population limits, terrain protection
- **Diplomacy**: Alliances, wars, rebellions, magical rites, border stealing
- **Civilizations**: Terramorphing, expansion, angry villagers, babies, migrants, armies
- **Units**: Gene Spaghetti, Mutant Box, hunger, old age
- **Mobs**: Peaceful monsters, animal babies, forever creep
- **Spawn**: Cloud of Life, animal spawn, sapient spawn
- **Nature**: Flora density, random seeds, minerals, erosion
- **Trees**: Growth, fast growth, entanglewood, bark bites back
- **Plants**: Growth, fast growth, tickles, root pranks, nectar nap
- **Fungi**: Growth, fast growth, exploding mushrooms
- **Biomes**: Grass growth, overgrowth
- **Weather**: Eternal lava, forever cold
- **Disasters**: Natural disasters, other disasters, rat king
- **Other**: Evolution events

---

## 4. UI/UX

### 4.1 Screen Layout
- **Top**: Minimal -- world name, population counter
- **Bottom**: Power tab bar (8 tabs), currently selected powers palette
- **Left side**: Kingdom/village info panels when selected
- **Right side**: Unit info panel when creature selected
- **Top-right**: Time controls (pause, speed), minimap toggle
- **Center**: The world view (main viewport, pan + zoom)

### 4.2 Power Bar
- Bottom of screen, horizontal tab bar
- Each tab opens a scrollable grid of power icons
- Icons are pixel art style, color-coded by function
- Hover shows tooltip with name + description
- Click to select, click on world to apply

### 4.3 Selection System
- **Click creature**: Opens Unit panel (stats, traits, equipment, family, mind)
- **Click village**: Opens Village panel (population, resources, buildings, storage)
- **Click kingdom**: Opens Kingdom panel (stats, wars, relations, villages, clans)
- **Hover unit** (Steam): Tooltip with basic info
- **Hover zone** (Steam): Tooltip with zone layer info

### 4.4 Unit Info Panel
When a creature is selected:
- Name, race, age, gender
- Stats: Health, Damage, Armor, Speed, Dodge, Accuracy, Diplomacy, Intelligence, Stewardship, Warfare
- Level and XP
- Traits list (clickable)
- Equipment (weapon, helmet, armor, ring, amulet)
- Profession/job
- Kingdom, village, culture, religion, language, clan, family
- Mood, hunger
- Kills count
- **Mind view**: Inner mind networks
- **Genealogy**: Parents, siblings, children, grandparents
- **Relationship arrows**: Lover, family, house, destination, attack target

### 4.5 Menu Structure
- **Main Menu**: New World, Load World, Settings
- **In-Game Menus**: World Laws, Ages, World History, Statistics, Achievements, Debug Menu
- **World Creation**: Map type (16 types), size (8 sizes), generation settings (noise, shapes, biomes, resources)
- **Debug Menu**: Developer console with advanced options (fast upgrades, fast construction, display overlays)

### 4.6 Settings
- Graphics quality
- Sound volume
- UI scale (toolbar size complaints exist)
- Language
- Autosave controls
- Performance options

---

## 5. PERFORMANCE

### 5.1 Map Sizes
| Name | Tiles | Grid |
|------|-------|------|
| Tiny | 128x128 | 2x2 |
| Small | 192x192 | 3x3 |
| Standard | 256x256 | 4x4 |
| Large | 320x320 | 5x5 |
| Huge | 384x384 | 6x6 |
| Gigantic | 448x448 | 7x7 |
| Titanic | 512x512 | 8x8 |
| Iceberg | 576x576 | 9x9 (PC only) |

- Modded maps up to 30x30 (1920x1920) playable on 16GB+ RAM
- 40x40-50x50 maps crash most systems

### 5.2 Population & Performance
- **Typical gameplay**: Hundreds to low thousands of units
- **Performance degrades** with high population + large maps
- **100 People world law** exists specifically to cap village pop for performance
- Community reports FPS drops and stuttering at high populations
- No official max unit count published -- practical limit ~5,000-10,000 depending on hardware
- Autosaves every 5 minutes, can be disabled on low-memory devices

### 5.3 Technical Implementation
- **Engine**: Unity (2D)
- **Rendering**: Sprite-based 2D, tile grid
- **Save format**: Compressed JSON (zlib), with full world state serialization
- **Platforms**: PC (Windows/Mac/Linux), iOS, Android
- **Data model**: Units have ~30+ serialized properties each (position, traits, stats, equipment, relationships, profession, hunger, mood, etc.)

---

## 6. WHAT WORLDBOX IS MISSING (Our Differentiators)

### 6.1 No Emotional Intelligence
- WorldBox creatures have **stats but no feelings**
- No happiness model beyond a simple mood string
- No grief when family members die
- No joy from achievements or relationships
- No fear of death or danger
- **Our advantage**: Full Maslow hierarchy of needs, emotional state machine, mood affects decision-making

### 6.2 No Relationship System
- WorldBox tracks family (parents, children, siblings) but has **no relationship depth**
- No love/romance mechanics (Smooth Jazz just forces breeding)
- No friendship formation
- No grudges or revenge
- No loyalty based on personal experience
- Lover Arrow exists but lover relationships have no gameplay impact
- **Our advantage**: Deep relationship graph with love, friendship, rivalry, revenge, mentorship, trust/betrayal dynamics

### 6.3 No Causal Memory
- WorldBox creatures **cannot remember events**
- No memory of being attacked, helped, or betrayed
- No learning from experience
- "Black Dust makes creatures forget everything" -- implying they had nothing meaningful to forget
- World History logs events but creatures don't reference them
- **Our advantage**: Episodic memory system where beings remember key events, learn from them, form opinions based on personal history

### 6.4 No Inner Life
- WorldBox has a "Mind" view but it shows **task networks, not thoughts**
- No decision-making visible to player
- No goals beyond immediate task (build, fight, eat)
- No dreams, aspirations, or fears
- No personality beyond trait stat modifiers
- **Our advantage**: Visible inner monologue, thought bubbles with actual reasoning, observable decision-making process

### 6.5 No Consequence Awareness
- Creatures don't understand consequences of actions
- No risk assessment before combat
- No consideration of family when going to war
- No economic understanding
- **Our advantage**: Beings weigh consequences, consider risk/reward, make trade-offs visible to the player

### 6.6 No Cultural Depth
- WorldBox has cultures, languages, religions as **stat modifiers and zone overlays**
- But they don't affect behavior meaningfully
- No cultural values that shape decisions
- No religious beliefs that constrain behavior
- No cultural memory or traditions
- **Our advantage**: Cultures as emergent behavioral patterns, religions with actual belief systems, traditions that persist and evolve

### 6.7 No Emergent Social Structures
- Kingdom structure is fixed: King -> Villages -> Citizens
- No merchant class, no guilds, no councils
- No social mobility beyond becoming king
- No economic specialization
- **Our advantage**: Emergent social hierarchies, economic classes, guilds, councils, social mobility based on merit and relationships

### 6.8 No Death Meaning
- Death in WorldBox is instant and forgotten
- No mourning, no funerals, no legacy
- No impact on survivors beyond population count
- **Our advantage**: Death has ripple effects -- grief, inheritance, power vacuums, revenge quests, memorial traditions

### 6.9 Summary: WorldBox vs. Swarm-OS

| Dimension | WorldBox | Swarm-OS |
|-----------|----------|----------|
| Beings | Stat bags with sprite | Sentient agents with inner life |
| Memory | None | Episodic + semantic |
| Emotions | Mood string | Full emotional model (Maslow + PAD) |
| Relationships | Family tree only | Love, trust, rivalry, revenge graph |
| Decisions | Task queue | Weighted multi-factor reasoning |
| Culture | Zone overlay | Emergent behavioral patterns |
| Death | Population -1 | Grief, legacy, power vacuum |
| Player insight | Stats panel | Thought bubbles, mind view, decision trace |
| Learning | None | Experience shapes future behavior |
| Consequence | None | Risk assessment, trade-off reasoning |

---

## 7. TECHNICAL DATA FOR ARCHITECTURE MATCHING

### 7.1 Entity Data Model (from WorldBox save format)
Each creature (actor) serializes:
```
x, y, gender, skin, skin_set, culture, clan, asset_id, profession,
kills, age_overgrowth, children, hunger, level, experience,
diplomacy, intelligence, stewardship, warfare, traits[], health,
name, created_time, mood, favoriteFood, items[], cityID
```

### 7.2 Kingdom Data Model
```
name, motto, kingID, capitalID, raceID, colorID, banner_background_id,
banner_icon_id, cultureID, royal_clan_id, deaths, born,
timestamp_alliance, timestamp_last_war, timestamp_new_conquest
```

### 7.3 City Data Model
```
kingdomID, leaderID, zones[], culture, race, deaths, born,
timer_supply, storage{resources[], weapons[], helmets[], armor[],
rings[], amulets[]}, pop_points
```

### 7.4 Key Simulation Parameters
- Village zone size: 8x8 tiles
- Minimum island size for village: 300 connected land tiles
- Farm radius: 9 tiles from windmill
- Building planning cooldown: 1 month
- Capture progress update: every 0.02 months
- Watchtower occupying force: +10
- Soldier occupying force: +1
- XP system: 1 xp per hit taken, 2 per hit dealt, 10 per kill
- Equipment cost: 3 iron per item
- Town hall upgrades: Tier 2 (10 wood, 10 stone), Tier 3 (10 wood, 10 stone, 10 iron)
- House upgrades: Tier 2 (4 wood), Tier 3 (4 wood, 4 stone), Tier 4 (10 wood, 10 stone), Tier 5-6 (10 wood, 10 stone, 10 iron, 10 gold)

---

## 8. FEATURE COUNT SUMMARY

| Category | Count |
|----------|-------|
| Total Powers | ~374 |
| Power Tabs | 8 |
| Creature Types | 118 |
| Creature Traits | 116 |
| Subspecies Traits | 204 |
| Biome Types | 31 |
| Wall Types | 7 |
| Ages/Eras | 10 |
| World Laws | 46+ |
| Map Sizes | 8 (+ modded) |
| Map Types | 16 |
| Building Types | ~10 per race |
| Cloud Types | 9+ |
| Bomb/Explosive Types | 12 |
| Civilized Races | 4 |
| World Gen Settings | ~15 |

---

*Research completed 2026-03-31. Sources: Official WorldBox Wiki (Fandom), WorldBox Wiki, Steam Community, Reddit r/Worldbox, zoosware.com, ancientsocieties.net, superworldbox.com*
