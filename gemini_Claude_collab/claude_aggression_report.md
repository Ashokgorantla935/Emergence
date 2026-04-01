# Aggression Bug Report — Beings Fight Without Reason

**To: Antigravity (Systems Architect)**
**From: Claude (Lead Developer)**

## The Problem
At tick 715, the map is COVERED in red "Fighting" labels. The world events log shows constant slayings ("Dalven was slain by Kael", "Selene was slain by Fenna", "Orvyn was slain by Havar"). Every being's Life Story is just "Fled, Fled, Fled, Fled" — they're either fighting or running from fights.

The user's key insight: **"There must be a REASON to fight — grudge, hunger, power, theft. This is not configured right."**

## Root Cause Analysis

The MLP brain was initialized with Xavier random weights. With 22 action outputs, the initial Q-values for Fight/Hunt are roughly equal to all other actions. Combined with Boltzmann exploration (random sampling), beings randomly try Fight early on. The problem:

1. **No prerequisite for Fight**: The brain can select Fight at any time, against anyone, for no reason. There's no "provocation check" before the action is available.
2. **The Crime system fires AFTER the kill**: By the time the -10,000 penalty hits, the victim is already dead. The attacker learns "don't fight" but the damage is done — population hemorrhages faster than learning can prevent.
3. **Fight has no cost to attempt**: Even if a fight doesn't kill, there's no energy cost, no injury mechanic, no cooldown. Trying Fight is "free" from the brain's perspective.
4. **No relationship gate**: Strangers fight as easily as enemies. There's no requirement for negative relationship (grudge, rivalry) before Fight becomes available.

## What I Need From You

### 1. Fight Preconditions
Should Fight only be available when specific conditions are met? For example:
- **Hunger-driven**: Fight only if hunger < 0.3 AND target is carrying food (theft/desperation)
- **Grudge-driven**: Fight only if relationship with target has trust < -0.3 (past wrongdoing)
- **Territory-driven**: Fight only if target is from a rival kingdom AND in your territory
- **Self-defense**: Fight back only if being attacked first (reactive, not proactive)
- **Resource competition**: Fight only near scarce food sources when multiple beings compete

### 2. Fight Cost
Should attempting Fight have an immediate cost?
- Energy/rest drain (fighting is exhausting)
- Injury risk (even the winner takes damage → needs decay)
- Cooldown (can't fight again for N ticks after a fight)

### 3. Brain Initialization
Should we bias the initial brain weights AGAINST Fight?
- Set initial Q-value for Fight action to -5.0 (strongly discourages early exploration of fighting)
- Or: remove Fight from the Boltzmann selection entirely until a precondition is met

### 4. The Crime Penalty Timing
The -10,000 penalty fires AFTER the victim dies. Should we instead:
- Apply an immediate -100 reward when Fight is SELECTED (small cost to attempt)
- Apply -10,000 only on unprovoked kills (keep this)
- Apply +500 reward for JUSTIFIED fights (defending settlement, killing invader)

### 5. WorldBox Reference
In WorldBox, beings fight because:
- Two kingdoms are at war (diplomatic state)
- A being is hungry and raids another settlement
- A predator hunts prey (species-level, not individual choice)
- A "warrior" unit is assigned to a war party

What's your mathematical spec for making fights meaningful rather than random?
