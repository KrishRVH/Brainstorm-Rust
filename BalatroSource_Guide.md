# BalatroSource Guide

This guide is derived directly from the in-repo `BalatroSource/` code. Every
point below is backed by the referenced source files; do not extend this guide
without verifying in `BalatroSource/`. It is the project source-code guide, not
only an optimization guide. Source details are welcome when they are accurate,
useful for future mods, and cited.

This revision was built from a six-agent read-only audit plus local
spot-checking of the cited files. `BalatroSource/` is the literal game source and
must not be committed to git.

## Citation Style

- File citations use `path` plus function/table names instead of line numbers
  where possible. Line numbers drift quickly in this source tree.
- If a behavior depends on a center `name`, tag `name`, blind `name`, or other
  string branch, cite the runtime function that checks it, not only the data
  definition.
- Treat this file as a map for future source reading. It should capture useful,
  vetted facts, not every branch in every Joker.

## Source Map

- `main.lua`: load order, LOVE callbacks, custom `love.run`, update/draw entry
  points, controller input forwarding.
- `globals.lua`: `VERSION`, feature flags, `G.SETTINGS`, profiles, state enums,
  instance registries, `G.UIT`, colors, hand ordering, input mappings.
- `game.lua`: startup, item prototype construction, profile loading,
  localization/render setup, run object shape, run start, state dispatch, shop
  population, draw pipeline, save flushing.
- `functions/misc_functions.lua`: PRNG, poker-hand detection, save helpers,
  localization parser, profile/stat helpers, card construction helpers.
- `functions/common_events.lua`: events and shared gameplay helpers, card
  evaluation dispatch, pool filtering, voucher/tag/pack selection, card creation,
  boss selection, UI card generation.
- `functions/state_events.lua`: round lifecycle, card movement, discard/play
  flows, poker-hand selection, scoring pipeline, round evaluation.
- `functions/UI_definitions.lua`: most UIBox definition builders, shop UI,
  blind-select UI, collection/run setup/profile screens, reusable UI helpers.
- `functions/button_callbacks.lua`: `G.FUNCS` UI validators and action callbacks:
  buy/use/sell/reroll/select blind/skip/cash out/run setup/settings.
- `card.lua`: runtime `Card` model, abilities, editions, seals, consumable
  effects, booster opening, voucher redeem/apply, Joker calculation, card
  draw/click/save/load.
- `cardarea.lua`: runtime containers for deck/hand/play/discard/jokers/shop,
  highlight rules, layout, shuffle/sort, save/load.
- `back.lua`: deck/back runtime behavior and special scoring/eval hooks.
- `blind.lua`: blind runtime behavior, boss setup, debuffs, hand rejection,
  disable rollback, save/load.
- `tag.lua`: tag runtime contexts, trigger effects, HUD/UI representation,
  save/load.
- `challenges.lua`: challenge definitions consumed by `Game:start_run`.
- `engine/*.lua`: object model, transform/movement, UI layout, events,
  controller, sprites/text/particles, save/sound/http manager threads.
- `localization/*.lua`: per-locale description and misc dictionaries.

Primary sources: all files listed above.

## Startup and Global Lifecycle

- `conf.lua` sets release/demo flags, window title, console flag, and initial
  window constraints. Runtime version is set in `globals.lua` as `1.0.1o-FULL`;
  `version.jkr` carries release metadata but is not read by the audited startup
  path. Sources: `conf.lua`; `globals.lua`; `version.jkr`.
- `main.lua` requires the engine and gameplay files, including `game.lua` before
  `globals.lua`; `globals.lua` instantiates `G = Game()`, and `Game:init()`
  assigns `G = self` before calling `Game:set_globals()`. Sources: `main.lua`;
  `globals.lua`; `game.lua` (`Game:init`).
- `love.run()` is customized. It calls `love.load()`, pumps events, smooths dt
  as `dt_smooth`, calls `love.update(dt_smooth)`, calls `love.draw()`, presents,
  and sleeps to `G.FPS_CAP` when frame time allows. Source: `main.lua`
  (`love.run`).
- `love.load()` calls `G:start_up()`, initializes Steam on Windows/macOS, and
  hides the OS cursor because the controller renders its own cursor. Source:
  `main.lua` (`love.load`).
- `Game:start_up()` loads settings, initializes the window, starts sound/save
  manager threads, optionally starts HTTP, loads shaders, initializes controller,
  loads the selected player profile, loads atlases/render settings, sets
  language, builds prototypes/pools, builds shared sprites, creates the cursor,
  creates the event manager, computes profile progress, and enters splash.
  Source: `game.lua` (`Game:start_up`).
- `Game:update()` is the frame backbone: timers, sound modulation, canvas juice,
  `EventManager:update`, state dispatch, animations, movement, object updates,
  controller update, localized blind-state refresh, Steam stat flush, debug
  counters, and queued save flush. Source: `game.lua` (`Game:update`).
- `Game:draw()` resets `G.DRAW_HASH`, draws instance registries in layered
  order, draws drag/focus/popup/alert/cursor layers, renders to `G.CANVAS`, then
  applies the CRT shader to `G.AA_CANVAS` before presenting to screen. Source:
  `game.lua` (`Game:draw`).

## Global State

- `G` is a global runtime singleton. `Game:set_globals()` initializes feature
  flags, timers, settings, metrics, three profile slots, render constants,
  state enums, stage/object registries, asset atlas tables, input mappings,
  colors, and the poker hand ordering. Source: `globals.lua`
  (`Game:set_globals`).
- `G.SETTINGS` holds user settings and defaults for language, sound, graphics,
  window mode, achievements, rumble, gameplay speed, pause state, and demo
  metadata. Source: `globals.lua`.
- `G.PROFILES` is a three-slot player-profile table. The active profile is
  selected by `G.SETTINGS.profile`; `Game:load_profile()` merges saved profile
  data into the default shape. Source: `globals.lua`; `game.lua`
  (`Game:load_profile`).
- `G.ARGS` is scratch/reuse storage, `G.FUNCS` is the global function/callback
  table, and `G.I` contains instance buckets for `NODE`, `MOVEABLE`, `SPRITE`,
  `UIBOX`, `POPUP`, `CARD`, `CARDAREA`, and `ALERT`. Sources: `globals.lua`;
  `engine/node.lua`; `engine/ui.lua`; `card.lua`; `cardarea.lua`.
- State constants are integers in `G.STATES`: selecting hand, hand played, draw
  to hand, game over, shop, play tarot, blind select, round eval, Tarot,
  Celestial, Spectral, Standard, Buffoon packs, menu, tutorial, splash, sandbox,
  demo CTA, and new round. Stages are main menu, run, and sandbox. Source:
  `globals.lua`.
- UI node tags live in `G.UIT`: `T` text, `B` box, `C` column, `R` row, `O`
  object, `ROOT`, `S` slider, `I` input, plus default `padding`. Source:
  `globals.lua`.

## Engine Primitives

- The OOP model is a small global class stack. `Object:extend()` creates
  subclasses, `Object:__call()` constructs objects and calls `init`, and
  `Object:is()` walks metatables for type checks. Source:
  `engine/object.lua`.
- `Node` owns transform `T`, collision transform `CT`, state flags, children,
  container, unique ID, frame bookkeeping, and stage-object registration. Nodes
  do not collide by default. Source: `engine/node.lua` (`Node:init`).
- `Moveable` extends `Node` with visible transform `VT`, velocities, role-based
  attachment/alignment, movement easing, parallax, and `juice_up` animation.
  Source: `engine/moveable.lua`.
- `UIBox` is the root of table-driven UI. It consumes a definition tree of
  `G.UIT` nodes, creates `UIElement`s, calculates widths/heights, aligns to a
  major object, and registers in `G.I.UIBOX` or a custom instance bucket.
  Source: `engine/ui.lua` (`UIBox:init`).
- UI action wiring is data-driven. A UI node `config.button` calls
  `G.FUNCS[button]`; `config.func` is commonly a live validator/updater; `id`
  makes a node discoverable through `get_UIE_by_ID`; `ref_table`/`ref_value`
  wire dynamic text; `focus_args` wires controller navigation and pips. Sources:
  `engine/ui.lua`; `functions/UI_definitions.lua`; `functions/button_callbacks.lua`.
- `Event` supports `immediate`, `after`, `before`, `ease`, and `condition`.
  Events default to `blocking = true` and `blockable = true`. If created while
  paused, the default timer is `REAL`; otherwise it is `TOTAL`, unless `timer`
  is explicitly set. Source: `engine/event.lua` (`Event:init`, `Event:handle`).
- `EventManager` has `unlock`, `base`, `tutorial`, `achievement`, and `other`
  queues. Blocking events prevent later blockable events in the same queue from
  advancing; non-blockable events can still run. Source: `engine/event.lua`
  (`EventManager:update`).
- The controller builds cursor collision from `G.DRAW_HASH`. Interactive custom
  drawables must be drawn through normal draw paths or otherwise call
  `add_to_drawhash()` to participate in hover/click collision. Sources:
  `engine/controller.lua`; `functions/misc_functions.lua` (`add_to_drawhash`);
  `game.lua` (`Game:draw`).
- `Sprite` and `AnimatedSprite` are `Moveable`s backed by atlases/quads.
  `Sprite:draw_shader()` is the main rendering extension point for dissolve,
  edition, voucher, booster, negative, and other shader passes. Sources:
  `engine/sprite.lua`; `engine/animatedsprite.lua`.
- `DynaText` is animated text with ref-table updates, pop-in/out, pulse, quiver,
  float, and sound ticks. It is commonly embedded with `G.UIT.O`. Source:
  `engine/text.lua`.
- `Particles` is a `Moveable` with attach/fill/timer/lifespan/fade behavior and
  is used by effects such as blind defeat. Source: `engine/particles.lua`;
  `blind.lua` (`Blind:defeat`).

## Save, Load, Profiles, and Unlocks

- Saves are packed Lua tables and deflate-compressed. `STR_PACK()` serializes
  tables and replaces live `Object` instances with `"MANUAL_REPLACE"`; this is
  why mod save state should be plain data, not live objects. Sources:
  `engine/string_packer.lua`; `functions/misc_functions.lua` (`save_run`).
- `save_run()` serializes all global `CardArea` instances, tags, `G.GAME`,
  `G.STATE`, pending `ACTION`, current `BLIND`, selected `BACK`, and `VERSION`.
  Arbitrary globals are not saved unless they are reachable from those tables.
  Source: `functions/misc_functions.lua` (`save_run`).
- `engine/save_manager.lua` is a thread that consumes typed requests:
  `save_progress`, `save_settings`, `save_metrics`, `save_notify`, and
  `save_run`. It writes `settings.jkr`, `<profile>/profile.jkr`,
  `<profile>/meta.jkr`, `<profile>/unlock_notify.jkr`, and
  `<profile>/save.jkr`. Source: `engine/save_manager.lua`.
- `Game:update()` flushes queued saves on force, stage change, pause transition
  for run saves, or after `G.F_SAVE_TIMER`. Source: `game.lua` (`Game:update`).
- Continue loads `<profile>/save.jkr` into `G.SAVED_GAME`, rejects old saves
  below the expected version gate, then calls `Game:start_run({savetext = ...})`
  to rebuild runtime objects. Sources: `functions/button_callbacks.lua`;
  `game.lua` (`Game:start_run`).
- Unlock/discovery metadata uses UDA flags in `meta.jkr` for centers, blinds,
  tags, and seals. `unlock_card()` and `discover_card()` return early in seeded
  or challenge runs. Back unlocks auto-discover the Back. Sources:
  `functions/common_events.lua` (`unlock_card`, `discover_card`);
  `game.lua` (`Game:init_item_prototypes`, `Game:save_progress`).
- Challenge completion is a challenge-mode exception: `win_game()` marks
  `G.PROFILES[profile].challenge_progress.completed[challenge]`, updates
  challenge unlock progress, and saves. Source: `functions/state_events.lua`
  (`win_game`).
- `BalatroSource/engine/profile.lua` is a debug profiler using `debug.sethook`;
  it is not player-profile persistence. Source: `engine/profile.lua`.

## Registries, Centers, and Pools

- `Game:init_item_prototypes()` is the root registry builder. It defines
  `P_SEALS`, `P_TAGS`, `P_STAKES`, `P_BLINDS`, `P_CARDS`, and `P_CENTERS`, then
  mutates entries with `.key`, unlock/discovery/alert state, and pool
  membership. Source: `game.lua` (`Game:init_item_prototypes`).
- `P_CARDS` are playing-card front prototypes keyed like `H_A`, with `name`,
  canonical English `value`, `suit`, and atlas `pos`. Runtime rank/suit logic
  reads these fields directly; localized display strings are in
  `localization/*.lua`. Sources: `game.lua` (`P_CARDS`); `card.lua`
  (`Card:set_base`); `localization/en-us.lua`.
- `P_CENTERS` contains gameplay centers: Jokers, Tarot, Planet, Spectral,
  Voucher, Back, Enhanced, Edition, Booster, Default, Demo, and extras like
  `soul`/undiscovered placeholders. Common fields include `order`, `unlocked`,
  `discovered`, `rarity`, `cost`, `name`, `pos`, `set`, `effect`, and `config`;
  fields are set-specific. Source: `game.lua` (`P_CENTERS`).
- Jokers use rarity, compatibility flags, optional unlock conditions, optional
  pool flags, and special fields such as `enhancement_gate` and `soul_pos`.
  Legendary Jokers have rarity `4`; `get_current_pool()` allows rarity 4 when
  `_legendary` is true even if normally locked. Sources: `game.lua`
  (`P_CENTERS` Joker entries); `functions/common_events.lua`
  (`get_current_pool`).
- Consumables are centers with `consumeable = true`. Tarot and Planet both use
  `c_` keys, so `set` distinguishes behavior and localization. Planet centers
  have `config.hand_type`; secret planets use `config.softlock`. Source:
  `game.lua` (`P_CENTERS` consumables); `functions/common_events.lua`
  (`get_current_pool`).
- Boosters are concrete centers with `kind`, `weight`, `cost`, `atlas =
  'Booster'`, and `config.extra`/`config.choose`. Runtime pack family detection
  in `Card:open()` uses `self.ability.name:find(...)`, not `kind` or key.
  Sources: `game.lua` (`P_CENTERS` booster entries); `card.lua`
  (`Card:open`).
- `P_CENTER_POOLS` is predeclared by set, and `P_JOKER_RARITY_POOLS` is four
  rarity arrays. Non-Joker centers skip pools when `wip`, `skip_pool`, or
  `omit`; consumables also enter `Consumeables`; Tarot and Planet also enter
  `Tarot_Planet`; Jokers with rarity enter the rarity pools. Source: `game.lua`
  (`Game:init_item_prototypes`).
- Most pools sort by `order`. Back sorting subtracts 100 from unlocked decks so
  unlocked decks appear first. Modded pooled centers should provide stable
  numeric `order`. Source: `game.lua` (`Game:init_item_prototypes`).
- `get_current_pool()` preserves source pool positions by inserting
  `'UNAVAILABLE'` placeholders for filtered entries. Callers resample if they
  hit a placeholder. Compacting pools changes deterministic behavior. Source:
  `functions/common_events.lua` (`get_current_pool`, `get_next_voucher_key`,
  `get_next_tag_key`, `create_card`).
- Duplicate center names are dangerous. `Card:set_ability()` marks
  `G.GAME.used_jokers[k]` by matching `v.name == self.ability.name`, and some
  save/load paths resolve by name. Keep keys and runtime names unique unless you
  have audited every name-based branch. Sources: `card.lua` (`Card:set_ability`);
  `blind.lua` (`Blind:save`); `functions/common_events.lua`
  (`get_deck_from_name`).

## Localization

- Locale files return `{ descriptions = {...}, misc = {...} }`. The English
  file contains description groups for Back, Blind, Edition, Enhanced, Joker,
  Other, Planet, Spectral, Stake, Tag, Tarot, Voucher, plus misc dictionaries.
  Source: `localization/en-us.lua`; sampled `fr.lua`, `ja.lua`, `zh_CN.lua`.
- `Game:set_language()` loads the selected locale; `init_localization()` parses
  description `text`, `name`, and optional `unlock` lines into parsed forms.
  Source: `game.lua` (`Game:set_language`); `functions/misc_functions.lua`
  (`init_localization`, `loc_parse_string`).
- `localize()` indexes `G.localization.descriptions[set][key]` directly for
  description/unlock/name lookups. Missing localization keys can crash or show
  `ERROR` depending on the call path. Source: `functions/misc_functions.lua`
  (`localize`).
- Center `name` is runtime data, not just display text. Logic branches on
  English names for special card shapes, Joker behavior, pack families, deck
  effects, blind effects, and tooltip variables. Localized display names belong
  in localization files; do not casually translate or rename `center.name`.
  Sources: `card.lua` (`Card:set_ability`, `Card:open`,
  `Card:calculate_joker`); `back.lua`; `blind.lua`; `tag.lua`;
  `functions/common_events.lua` (`generate_card_ui`).
- Booster localization is aliased through `descriptions.Other.p_*` family keys,
  not every concrete booster key. Seals use `P_SEALS` keys like `Gold`, while
  descriptions/labels are in `Other.gold_seal`, etc.; there is no dedicated
  `descriptions.Seal` group in `en-us`. Source: `localization/en-us.lua`;
  `game.lua` (`P_SEALS`).

## Run Object and Run Start

- `Game:init_game_object()` returns the run state `G.GAME`: scores/usages,
  modifiers, `starting_params`, `banned_keys`, probabilities, pseudorandom
  state, rates, tags, used Jokers/vouchers, current round state, round reset
  state, shop state, card-play tallies, and poker-hand definitions. Source:
  `game.lua` (`Game:init_game_object`).
- Default shop rates are Joker 20, Tarot 4, Planet 4, Spectral 0, playing card
  0. Default `G.GAME.shop.joker_max` is 2. Source: `game.lua`
  (`Game:init_game_object`).
- `Game:start_run(args)` accepts an optional save table. Without a save, it
  applies stake modifiers, applies the selected Back, applies challenge rules,
  initializes starting params, sets seed state, chooses boss/voucher/blind tags,
  creates card areas, builds the deck, shuffles, and builds HUD/blind runtime
  objects. Sources: `game.lua` (`Game:start_run`); `back.lua`
  (`Back:apply_to_run`).
- If `args.seed` exists, `G.GAME.seeded = true`. The seed is `args.seed`,
  `"TUTORIAL"`, or `generate_starting_seed()` depending on tutorial state.
  `G.GAME.pseudorandom.hashed_seed` is `pseudohash(seed)`. Source: `game.lua`
  (`Game:start_run`); `functions/misc_functions.lua`.
- Non-save start-roll order is boss, voucher, Small tag, Big tag:
  `get_new_boss()`, `get_next_voucher_key()`,
  `get_next_tag_key()`, `get_next_tag_key()`. Tutorial progress can force the
  voucher/tags. Source: `game.lua` (`Game:start_run`).
- Card areas created at run start include `G.consumeables`, `G.jokers`,
  `G.discard`, `G.deck`, `G.hand`, and `G.play`. `G.playing_cards` is rebuilt
  from saves or generated from deck prototypes. Source: `game.lua`
  (`Game:start_run`).
- Challenge decks can override the selected Back with `args.challenge.deck.type`
  and can provide exact deck card lists. Without exact lists, deck generation
  starts from `P_CARDS`, applies challenge/back restrictions, applies no-face
  filtering, sorts card prototypes, then calls `card_from_control()`. Sources:
  `game.lua` (`Game:start_run`); `functions/misc_functions.lua`
  (`card_from_control`); `challenges.lua`.

## State Flow and Round Flow

- `Game:update()` dispatches by `G.STATE` to handlers such as `update_shop`,
  `update_blind_select`, `update_draw_to_hand`, `update_hand_played`,
  pack-state handlers, `update_round_eval`, and `update_game_over`. Entry work
  is usually gated by `G.STATE_COMPLETE == false`. Source: `game.lua`
  (`Game:update`).
- Typical run loop:
  `BLIND_SELECT` -> `select_blind` -> `new_round()` -> `DRAW_TO_HAND` ->
  `SELECTING_HAND` -> play/discard callbacks -> `HAND_PLAYED` ->
  `DRAW_TO_HAND` or `NEW_ROUND` -> `end_round()` -> `ROUND_EVAL` ->
  `cash_out` -> `SHOP` -> `toggle_shop` -> `BLIND_SELECT`.
  Sources: `functions/button_callbacks.lua`; `functions/state_events.lua`;
  `game.lua`.
- `new_round()` resets hands/discards/reroll counters, clears `used_packs`,
  resets per-round hand played counters, sets blind state/current type, calls
  `Blind:set_blind()`, fires Joker `{setting_blind = true}`, then transitions to
  `DRAW_TO_HAND` and shuffles the deck with `nr<ante>`. Source:
  `functions/state_events.lua` (`new_round`).
- `end_round()` evaluates game over/win, fires end-of-round Joker effects,
  rental/perishable ticks, discovery/unlock/stat updates, end-of-round held-card
  effects, moves hand/discard back to deck, transitions to `ROUND_EVAL`, and if
  a boss was defeated refreshes `G.GAME.current_round.voucher`. Source:
  `functions/state_events.lua` (`end_round`).
- `cash_out` pays round dollars, resets shop/hand counters, sets
  `G.STATE = SHOP`, clears shop flags, refreshes Small/Big tags after a boss
  defeat, calls `reset_blinds()`, and resets chips. Source:
  `functions/button_callbacks.lua` (`G.FUNCS.cash_out`).
- `toggle_shop` runs Joker `{ending_shop = true}` hooks, slides/removes shop UI,
  then sets state back to `BLIND_SELECT`. Source: `functions/button_callbacks.lua`
  (`G.FUNCS.toggle_shop`).

## Card Model and Card Areas

- `Card:init()` creates `self.config.card/center`, initializes costs and child
  sprites, sets `edition = nil`, calls `set_ability(center, true)` before
  `set_base(card, true)`, starts with `area = nil`, `highlighted = false`,
  `debuff = false`, and registers in `G.I.CARD`. Source: `card.lua`
  (`Card:init`).
- `Card:set_base()` maps a `P_CARDS` prototype to `config.card_key`, builds
  `self.base`, maps ranks 2 through Ace to ids 2 through 14, assigns nominal
  values, suit nominals, color, and re-runs blind debuff/unlock checks when not
  initial. Source: `card.lua` (`Card:set_base`).
- `Card:set_ability()` maps a `P_CENTERS` prototype to `config.center_key`,
  rebuilds `self.ability` from `center.config`, preserves `forced_selection` and
  `perma_bonus`, attaches `ability.consumeable`, sets labels, applies special
  shape branches for some Jokers, and re-runs blind debuff when not initial.
  Source: `card.lua` (`Card:set_ability`).
- `CardArea:init()` stores `cards`, `highlighted`, `card_limit`,
  `highlighted_limit`, `type`, sort mode, and card width. Default type is
  `deck`. Source: `cardarea.lua` (`CardArea:init`).
- `CardArea:emplace()` inserts at front for deck/front insertions and append
  otherwise, flips face-up except deck/discard/stay-flipped cases, calls
  `card:set_card_area(self)`, ranks/aligns, and triggers joker/deck unlock
  checks for relevant areas. Source: `cardarea.lua` (`CardArea:emplace`).
- `CardArea:remove_card()` selects from the end for deck/discard by default and
  from the front otherwise, clears the card area, removes highlights, reranks,
  and returns the card. Source: `cardarea.lua` (`CardArea:remove_card`).
- Highlighting is type-driven. Hand, Joker, consumeable, and shop areas can
  highlight; shop areas force one highlighted card. Hand highlighting previews
  hand text and calls `Blind:debuff_hand(..., check=true)` to show rejected
  hands. Source: `cardarea.lua` (`can_highlight`, `add_to_highlighted`,
  `parse_highlighted`).
- Layout is keyed by `CardArea.config.type`. Deck/discard flip cards down,
  hand/play/shop/joker/consumeable/voucher/title areas all have different
  alignment and sorting behavior. Source: `cardarea.lua` (`align_cards`).
- `CardArea:shuffle(seed)` calls `pseudoshuffle(self.cards, pseudoseed(seed or
  'shuffle'))`; `CardArea:draw_card_from()` removes from one area, asks the
  blind whether the card should stay flipped, handles challenge flipped-card
  modifiers, and emplaces into the destination. Source: `cardarea.lua`.

## Enhancements, Editions, and Seals

- Enhancements are center configs. Examples: Bonus `bonus=30`, Mult `mult=4`,
  Glass `Xmult=2, extra=4`, Steel `h_x_mult=1.5`, Stone `bonus=50`, Gold
  `h_dollars=3`, Lucky `mult=20, p_dollars=20`; Wild has empty config and is
  handled by suit checks. Source: `game.lua` (`P_CENTERS` Enhanced entries);
  `card.lua` helper methods.
- There is no single `calculate_enhancement` hook. Enhancement scoring routes
  through `Card` helper methods such as chip bonus/mult/x-mult/dollars,
  held-card helpers, and `eval_card()`. Sources: `card.lua`;
  `functions/common_events.lua` (`eval_card`).
- Editions are center configs but runtime effects live on `self.edition`. Foil
  sets chip bonus, Holographic sets mult, Polychrome sets x-mult, Negative sets
  `negative/type` and can increase Joker or consumeable slot limit when applied
  to an already-added card. Sources: `game.lua` (`P_CENTERS` Edition entries);
  `card.lua` (`Card:set_edition`).
- Seals are strings (`Gold`, `Purple`, `Red`, `Blue`) rendered via
  `G.shared_seals`. Red gives repetitions, Purple can create a Tarot on discard
  if there is room, Blue can create last-hand Planet at end of round if there is
  room, and Gold adds played-card dollars. Sources: `game.lua` (`P_SEALS`,
  shared seals); `card.lua` (`Card:set_seal`, `calculate_seal` and related
  seal paths).
- Debuffing a Joker calls `remove_from_deck`; undebuffing calls `add_to_deck`.
  Perishable at zero forces debuff and removes deck effects. Source: `card.lua`
  (`Card:set_debuff`, `calculate_perishable`).
- `add_to_deck()` returns early for Enhanced and Default playing cards, but
  Jokers can alter hand size, discards, probabilities, interest, blind state,
  and Negative slot limits. `remove_from_deck()` reverses those global effects.
  Source: `card.lua` (`Card:add_to_deck`, `Card:remove_from_deck`).
- Stone cards are special: `get_id()` returns a random negative id for
  non-vampired Stone Cards, and Stone cards are not suits. Wild cards match any
  suit unless debuff-specific calls block them; Smeared Joker changes suit
  matching by red/black color groups. Source: `card.lua` (`Card:get_id`,
  `Card:is_suit`).

## Card Creation, Consumables, and Packs

- `create_playing_card()` increments `G.playing_card`, creates a `Card`, inserts
  it into `G.playing_cards`, optionally emplaces it into a target area, and
  materializes it. Source: `functions/common_events.lua`
  (`create_playing_card`).
- `create_card(_type, area, legendary, _rarity, skip_materialize, soulable,
  forced_key, key_append)` is the main center factory. It supports forced keys,
  pool sampling, Soul/Black Hole injection, random playing-card fronts for
  Base/Enhanced, discovery bypass flags, Joker shop stickers, and Joker
  editions. Source: `functions/common_events.lua` (`create_card`).
- Soul injection only runs when `soulable` is true and the relevant card is not
  banned/blocked by Showman logic. Tarot/Spectral/Tarot_Planet can become
  `c_soul`; Planet/Spectral can become `c_black_hole`; both use
  `pseudorandom('soul_'.._type..ante) > 0.997`. Source:
  `functions/common_events.lua` (`create_card`).
- `poll_edition()` returns `{negative}`, `{polychrome}`, `{holo}`, `{foil}`, or
  nil. Guaranteed mode uses fixed 25x thresholds; normal mode is scaled by
  `G.GAME.edition_rate` for non-negative editions. Source:
  `functions/common_events.lua` (`poll_edition`).
- `Card:use_consumeable()` records usage before the debuff early return, then
  branches by name/config for conversions, seals, Aura, Cryptid, Sigil/Ouija,
  hand leveling, removal/creation, money, Joker creation, Ankh/Wraith/Wheel,
  Ectoplasm, Hex, Immolate, and other consumable effects. Source: `card.lua`
  (`Card:use_consumeable`).
- The UI use flow removes the card from its area, moves shop/pack/blind UI
  offscreen as needed, dispatches by set (consumeable, playing card, Joker,
  Booster, Voucher), then restores previous state/focus unless the pack remains
  active. Source: `functions/button_callbacks.lua` (`G.FUNCS.use_card`).
- `Card:open()` handles boosters by `self.ability.name:find(...)`: Arcana sets
  `TAROT_PACK`, Celestial sets `PLANET_PACK`, Spectral sets `SPECTRAL_PACK`,
  Standard sets `STANDARD_PACK`, Buffoon sets `BUFFOON_PACK`. It sets
  pack size/choices, charges cost, creates pack cards, emplaces them into
  `G.pack_cards`, and fires Joker `{open_booster = true}`. Source: `card.lua`
  (`Card:open`).
- Standard packs add random Base/Enhanced playing cards, then separately poll
  standard edition and seal chances. Sources: `card.lua` (`Card:open`);
  `functions/common_events.lua` (`create_card`, `poll_edition`).

## Shop Generation

- `G.UIDEF.shop()` builds three `CardArea`s: `G.shop_jokers` with limit
  `G.GAME.shop.joker_max`, `G.shop_vouchers` with limit 1, and
  `G.shop_booster` with limit 2. All are `type = 'shop'` and highlight limit 1.
  Source: `functions/UI_definitions.lua` (`G.UIDEF.shop`).
- Shop UI embeds the card areas through `G.UIT.O`, wires the next-round button
  to `toggle_shop`, and wires the reroll button to `reroll_shop` with validator
  `can_reroll`. Source: `functions/UI_definitions.lua` (`G.UIDEF.shop`).
- Normal stock population is in `Game:update_shop()`, not in `G.UIDEF.shop()`.
  The fill order is: apply `shop_start` tags, fill Joker/shop-card slots with
  `create_card_for_shop`, show/load current voucher, generate two booster slots
  using `get_pack('shop_pack')`, then apply `voucher_add` and `shop_final_pass`
  tags. Sources: `game.lua` (`Game:update_shop`);
  `functions/UI_definitions.lua` (`create_card_for_shop`);
  `tag.lua` (`Tag:apply_to_run`).
- `create_card_for_shop(area)` handles forced tutorial stock, tag-driven
  `store_joker_create` and `store_joker_modify`, weighted type polling by
  `joker_rate`, `tarot_rate`, `planet_rate`, `playing_card_rate`, and
  `spectral_rate`, then calls `create_card(..., key_append='sho')`. Source:
  `functions/UI_definitions.lua` (`create_card_for_shop`).
- Shop type polling uses `pseudorandom(pseudoseed('cdt'..ante)) * total_rate`.
  If `v_illusion` is used, the playing-card branch can choose Enhanced over Base
  via `pseudorandom(pseudoseed('illusion')) > 0.6`; Illusion can also add an
  edition with additional `illusion` rolls. Source:
  `functions/UI_definitions.lua` (`create_card_for_shop`).
- `create_shop_card_ui()` attaches price, buy/redeem/open, and optional
  buy-and-use UIBoxes to cards. Voucher/booster UI initially names buttons
  `redeem_from_shop`/`open_booster`, but validators `can_redeem`/`can_open`
  rewrite enabled buttons to `use_card`. Sources:
  `functions/UI_definitions.lua` (`create_shop_card_ui`);
  `functions/button_callbacks.lua` (`can_redeem`, `can_open`).
- `buy_from_shop` validates capacity, removes from shop area, calls
  `add_to_deck`, emplaces into deck/consumeables/jokers as appropriate, subtracts
  dollars, increments stats, fires buying-card Joker hooks, and optionally chains
  buy-and-use. Source: `functions/button_callbacks.lua`
  (`G.FUNCS.buy_from_shop`).
- `reroll_shop` only removes/refills `G.shop_jokers`. It does not reroll the
  voucher or booster slots. Source: `functions/button_callbacks.lua`
  (`G.FUNCS.reroll_shop`).

## Vouchers

- Voucher selection uses `get_next_voucher_key(_from_tag)`, which samples the
  `Voucher` pool from `get_current_pool('Voucher')` and resamples
  `'UNAVAILABLE'` entries. Voucher Tag changes the pool key to
  `Voucher_fromtag`. Sources: `functions/common_events.lua`
  (`get_next_voucher_key`, `get_current_pool`); `tag.lua`.
- Voucher pool filtering excludes used vouchers, checks prerequisite
  `requires`, excludes duplicates already in `G.shop_vouchers`, respects
  `banned_keys`, and inserts placeholders for excluded entries. Source:
  `functions/common_events.lua` (`get_current_pool`).
- `G.GAME.current_round.voucher` is set at run start, refreshed after boss
  defeat in `end_round()`, and used by `Game:update_shop()` to spawn the single
  normal voucher slot. Sources: `game.lua` (`Game:start_run`,
  `Game:update_shop`); `functions/state_events.lua` (`end_round`).
- Redeeming a voucher sets `G.GAME.used_vouchers[center_key] = true`, clears
  `G.GAME.current_round.voucher`, and applies runtime effects through
  `Card:apply_to_run()`. Source: `card.lua` (`Card:redeem`,
  `Card:apply_to_run`).
- `requires` is overloaded. Tags use a single center key requiring discovery;
  vouchers use a list of prerequisite voucher keys requiring redemption. Sources:
  `game.lua` (`P_TAGS`, `P_CENTERS` vouchers); `functions/common_events.lua`
  (`get_current_pool`).

## Booster Packs

- `get_pack(_key, _type)` first forces the first shop pack to a normal Buffoon
  pack if `G.GAME.first_shop_buffoon` is false and `p_buffoon_normal_1` is not
  banned; this uses unseeded `math.random(1, 2)` for `_1`/`_2`. Otherwise it
  weights `G.P_CENTER_POOLS.Booster` by `weight`, optional `kind`, and
  `banned_keys`, then polls with `pseudorandom(pseudoseed(key..ante))`. Source:
  `functions/common_events.lua` (`get_pack`).
- `G.GAME.current_round.used_packs` resets to `{}` in `new_round()`. When the
  shop opens, each of two booster slots stores a pack key if unset. Buying a
  booster marks its slot as `'USED'`. Sources: `functions/state_events.lua`
  (`new_round`); `game.lua` (`Game:update_shop`);
  `functions/button_callbacks.lua` (`G.FUNCS.use_card`).
- Balatro defines 32 concrete booster center keys. Family names and `kind`
  drive most behavior, but exact variants matter for art/key filters and some
  unseeded tag/first-pack variant choices. Source: `game.lua` (`P_CENTERS`
  booster entries); `tag.lua`; `functions/common_events.lua` (`get_pack`).
- Buffoon pack sizing from center configs: normal has `extra = 2`, `choose = 1`;
  jumbo has `extra = 4`, `choose = 1`; mega has `extra = 4`, `choose = 2`.
  Source: `game.lua` (`P_CENTERS` booster entries).
- Pack content key appends used by `create_card`: Arcana Tarot `ar1`, Arcana
  via Omen Globe Spectral `ar2`, Celestial `pl1`, Spectral `spe`, Standard
  `sta`, Buffoon `buf`. Source: `card.lua` (`Card:open`).
- Omen Globe lets Arcana pack cards become Spectral when
  `pseudorandom('omen_globe') > 0.8`. Telescope forces the first Celestial pack
  card to the Planet for the most-played visible hand, when such a hand exists.
  Source: `card.lua` (`Card:open`).
- Using The Soul calls `create_card('Joker', ..., legendary=true,
  key_append='sou')`. Legendary selection uses `get_current_pool('Joker',
  ..., _legendary=true)`, which picks from rarity pool 4 with pool key `Joker4`
  and no ante suffix; edition polling on that Joker uses `edi` + `sou` + ante.
  Sources: `card.lua` (`Card:use_consumeable`); `functions/common_events.lua`
  (`create_card`, `get_current_pool`).

## Scoring and Joker Contexts

- `evaluate_poker_hand()` computes the possible hand buckets and returns
  ordered results for Flush Five, Flush House, Five of a Kind, Straight Flush,
  Four of a Kind, Full House, Flush, Straight, Three of a Kind, Two Pair, Pair,
  and High Card. `G.FUNCS.get_poker_hand_info()` selects the highest bucket in
  that order and converts Royal Flush display text as a Straight Flush variant.
  Sources: `functions/misc_functions.lua` (`evaluate_poker_hand`);
  `functions/state_events.lua` (`G.FUNCS.get_poker_hand_info`).
- `eval_card()` dispatches played cards through chip bonus, mult, x-mult,
  played dollars, `calculate_joker`, and edition; held cards through held
  mult/x-mult and `calculate_joker`; Joker/consumeable scoring through edition,
  `other_joker`, or `calculate_joker`. Source: `functions/common_events.lua`
  (`eval_card`).
- `G.FUNCS.evaluate_play()` detects the hand, records usage, adds Stone cards
  to the scoring hand unless Splash is present, highlights scoring cards,
  checks `Blind:debuff_hand`, runs Joker `before`, lets blinds modify hand
  chips/mult, scores played-card effects/repetitions, scores held-card effects,
  scores Joker editions/main effects/Joker-on-Joker effects, applies selected
  Back final scoring, destroys cards, applies hand total, then runs Joker
  `after`. Source: `functions/state_events.lua` (`G.FUNCS.evaluate_play`).
- Verified Joker contexts include: `open_booster`, `buying_card`,
  `selling_self`, `selling_card`, `reroll_shop`, `ending_shop`, `skip_blind`,
  `skipping_booster`, `playing_card_added`, `first_hand_drawn`,
  `setting_blind`, `destroying_card`, `remove_playing_cards`,
  `using_consumeable`, `debuffed_hand`, `pre_discard`, `discard`,
  `end_of_round`, `individual`, `repetition`, `other_joker`, `before`,
  `after`, and `joker_main`. Sources: `card.lua` (`Card:calculate_joker`);
  `functions/common_events.lua` (`eval_card`);
  `functions/state_events.lua`; `functions/button_callbacks.lua`.
- Blueprint copies the next Joker to its right; Brainstorm copies the first
  Joker. Both increment `context.blueprint`, set `context.blueprint_card`, and
  stop if recursion exceeds `#G.jokers.cards + 1`. Source: `card.lua`
  (`Card:calculate_joker`).
- Several displayed or eligibility values are recomputed in `Card:update`,
  including Temperance money, Throwback x-mult, Driver's License tally,
  Steel/Stone/Cloud 9 tallies, editionless Joker pools, Blueprint/Brainstorm
  compatibility, and Swashbuckler mult. Source: `card.lua` (`Card:update`).

## Blinds

- Blind definitions are data entries in `G.P_BLINDS`; boss eligibility uses
  boss metadata, and common debuffs such as suit/face/hand-size gates live in
  each blind's `debuff`. Sources: `game.lua` (`P_BLINDS`);
  `functions/common_events.lua` (`get_new_boss`).
- Run start chooses the Boss blind key; blind selection stores the chosen blind
  in `G.GAME.round_resets.blind`; `new_round()` makes it current and calls
  `Blind:set_blind()`. Sources: `game.lua` (`Game:start_run`);
  `functions/button_callbacks.lua` (`G.FUNCS.select_blind`);
  `functions/state_events.lua` (`new_round`).
- `Blind:set_blind()` copies blind config, dollars, debuff, pos, mult, boss
  state, computes chips from ante amount x blind mult x back ante scaling,
  updates HUD text/colors, runs special setup for specific bosses, debuffs all
  playing cards, and debuffs Jokers when not reset. Source: `blind.lua`
  (`Blind:set_blind`).
- Boss behavior is spread across `press_play`, `debuff_hand`, `modify_hand`,
  `drawn_to_hand`, `stay_flipped`, and `debuff_card`. The scoring/draw loops
  call these at specific times rather than one central blind hook. Sources:
  `blind.lua`; `functions/state_events.lua`.
- `Blind:debuff_hand()` rejects hands by configured hand/size rules, The Eye
  repeated-hand rule, and The Mouth first-hand-only rule. The Arm and The Ox
  also mutate state/money when not in preview check mode. Source: `blind.lua`
  (`Blind:debuff_hand`).
- `Blind:debuff_card()` debuffs non-Joker cards by suit, face, Pillar
  ante-played state, value, nominal, or Verdant Leaf; Crimson Heart has a Joker
  branch that avoids normal clearing while active. Source: `blind.lua`
  (`Blind:debuff_card`).
- `Blind:disable()` is a rollback path for boss-disabling effects. It restores
  Water/Needle/Manacle state, clears forced selections, flips applicable cards,
  reduces Wall/Vessel requirements, re-runs debuffs, and can advance to
  `NEW_ROUND` if the now-disabled boss is already beaten. Source: `blind.lua`
  (`Blind:disable`).
- Blind save resolves `config_blind` by matching blind `name` to `G.P_BLINDS`;
  duplicate blind names can break save/load identity. Source: `blind.lua`
  (`Blind:save`, `Blind:load`).

## Backs and Decks

- Back definitions live as `P_CENTERS` entries with config-driven effects and
  unlock gates such as `discover_amount`, `win_deck`, and `win_stake`. Challenge
  Deck is `omit = true`. Source: `game.lua` (`P_CENTERS` Back entries).
- `Back:apply_to_run()` applies vouchers, consumables, dollars, hands,
  discards, reroll costs, spectral rates, no-face flag, joker/consumeable slots,
  hand size, ante scaling, no-interest/money modifiers, Erratic flag, and some
  named deck effects. Source: `back.lua` (`Back:apply_to_run`).
- Some Back behavior is not purely config-driven. Anaglyph adds a Double Tag
  after boss evaluation, and Plasma balances chips/mult in the final scoring
  step. Source: `back.lua` (`Back:trigger_effect`).
- Magic starts with Crystal Ball and Fool, Nebula starts with Telescope and a
  consumable-slot penalty, Ghost sets `spectral_rate = 2` and starts with Hex,
  Zodiac starts with Tarot Merchant, Planet Merchant, and Overstock, and Erratic
  sets `randomize_rank_suit`. Sources: `game.lua` (`P_CENTERS` Back entries);
  `back.lua` (`Back:apply_to_run`).
- Checkered Deck rewrites Clubs to Spades and Diamonds to Hearts after deck
  creation via an event over `G.playing_cards`. Source: `back.lua`
  (`Back:apply_to_run`).

## Tags

- Tag prototypes define `config.type`, `min_ante`, optional `requires`, order,
  and per-tag config. `Tag:init()` copies the prototype, assigns tally/ID, and
  resolves Orbital hand ability. Sources: `game.lua` (`P_TAGS`);
  `tag.lua` (`Tag:init`, `Tag:set_ability`).
- `Tag:apply_to_run(context)` fires once and only when
  `self.config.type == context.type`. Meaningful contexts include `eval`,
  `immediate`, `new_blind_choice`, `voucher_add`, `tag_add`,
  `round_start_bonus`, `store_joker_create`, `shop_start`,
  `store_joker_modify`, and `shop_final_pass`. Source: `tag.lua`
  (`Tag:apply_to_run`).
- Call sites are distributed: `add_tag()` triggers `tag_add`; shop creation
  triggers shop/store/voucher/final-pass contexts; draw-to-hand triggers
  `round_start_bonus`; round evaluation triggers `eval`; skip/reroll/blind
  choice flows trigger `immediate` and `new_blind_choice`. Sources:
  `functions/UI_definitions.lua` (`add_tag`); `game.lua` (`Game:update_shop`);
  `functions/state_events.lua`; `functions/button_callbacks.lua`.
- Pack-spawning tags use concrete center keys and some use unseeded
  `math.random` for variants: Charm picks `p_arcana_mega_1` or `_2`, Meteor
  picks `p_celestial_mega_1` or `_2`; Ethereal/Standard/Buffoon use fixed keys.
  Source: `tag.lua` (`Tag:apply_to_run`).
- Voucher Tag adds an extra voucher slot by increasing
  `G.shop_vouchers.config.card_limit`, sampling `get_next_voucher_key(true)`,
  and emplacing a new voucher card. Source: `tag.lua` (`Tag:apply_to_run`).

## Challenges

- Challenge records use `name`, `id`, `rules.custom`, `rules.modifiers`,
  `jokers`, `consumeables`, `vouchers`, `deck`, and `restrictions`. Source:
  `challenges.lua`.
- `Game:start_run()` applies challenge Jokers/consumeables/vouchers, copies
  `rules.modifiers` into `starting_params`, converts custom rules into
  `G.GAME.modifiers` or special cases like `no_shop_jokers`, and flattens
  restrictions into `G.GAME.banned_keys`. Source: `game.lua`
  (`Game:start_run`).
- Challenge restrictions flatten by `id`; `type = 'blind'` metadata in
  `banned_other` is for description/UI and does not create a separate runtime
  filter path. Sources: `challenges.lua`; `game.lua` (`Game:start_run`).
- Challenge unlock progression starts after 5 white-stake deck wins, then
  unlocks completed challenge count plus 5. Challenge wins mark profile
  challenge progress. Sources: `globals.lua` (`G.CHALLENGE_WINS`);
  `functions/misc_functions.lua` (`set_challenge_unlock`);
  `functions/state_events.lua` (`win_game`).

## PRNG and Seeds

- `pseudohash(str)` hashes a string to a float in `[0, 1)` using a fixed formula
  with `1.1239285023`, `math.pi`, and `% 1`. Source:
  `functions/misc_functions.lua` (`pseudohash`).
- `pseudoseed(key, predict_seed)` drives deterministic RNG:
  - If `key == 'seed'`, it returns `math.random()` directly.
  - If `predict_seed` is supplied, it hashes `key..predict_seed`, transforms it
    with constants `2.134453429141` and `1.72431234`, and averages with
    `pseudohash(predict_seed)`.
  - Otherwise it updates `G.GAME.pseudorandom[key]` in-place and returns an
    average with `G.GAME.pseudorandom.hashed_seed`.
  - Each call mutates the per-key state, so call order matters.
  Source: `functions/misc_functions.lua` (`pseudoseed`).
- `pseudorandom(seed, min, max)` converts string seeds through `pseudoseed`,
  calls `math.randomseed(seed)`, and returns `math.random()` or
  `math.random(min, max)`. Source: `functions/misc_functions.lua`
  (`pseudorandom`).
- `pseudorandom_element(_t, seed)` builds a deterministic ordering of `_t`: it
  sorts by `sort_id` if present on entries, otherwise by key; then it seeds
  `math.randomseed(seed)` and selects `math.random(#keys)`. The sorted order is
  part of the RNG result. Source: `functions/misc_functions.lua`
  (`pseudorandom_element`).
- `pseudoshuffle(list, seed)` seeds `math.randomseed(seed)` when seed exists,
  sorts by `sort_id` when entries have it, then runs a Fisher-Yates style
  backwards shuffle using `math.random(i)`. Source:
  `functions/misc_functions.lua` (`pseudoshuffle`).
- Pools intentionally keep `'UNAVAILABLE'` placeholders before sampling, then
  resample with `<pool_key>_resampleN` if selected. Native search tables must
  not compact unavailable entries unless they reproduce the same resampling
  distribution. Sources: `functions/common_events.lua` (`get_current_pool`,
  `get_next_voucher_key`, `get_next_tag_key`, `create_card`).
- `random_string(length, seed)` is used to generate user seeds. It seeds
  `math.randomseed(seed)` and builds an uppercase string from digits `1-9`,
  letters `A-N`, or `P-Z` (no `O`) based on `math.random()` thresholds. Source:
  `functions/misc_functions.lua` (`random_string`).
- `generate_starting_seed()`:
  - For stake >= 8, it uses profile data to split legendary Jokers into two
    sets based on `get_joker_win_sticker(v, true)`. If both sets are non-empty,
    it loops until `get_first_legendary(seed)` returns a key in the
    lower-sticker set.
  - Otherwise it returns `random_string(8, ...)` seeded with cursor position and
    time.
  Sources: `functions/misc_functions.lua` (`generate_starting_seed`,
  `get_first_legendary`, `get_joker_win_sticker`).

## Search-Relevant RNG Paths

- Joker/consumable shop cards use `create_card_for_shop()`, which polls type via
  `cdt<ante>` and calls `create_card(..., key_append='sho')`. Joker rarity uses
  `rarity<ante>sho`; Joker pool sampling uses `Joker<rarity>sho<ante>`.
  Sources: `functions/UI_definitions.lua` (`create_card_for_shop`);
  `functions/common_events.lua` (`create_card`, `get_current_pool`).
- Booster shop slots use repeated `get_pack('shop_pack')` calls; both advance
  the same `shop_pack<ante>` RNG key, unless the first-pack Buffoon force path
  applies. Sources: `game.lua` (`Game:update_shop`);
  `functions/common_events.lua` (`get_pack`).
- Standard pack card type uses `stdset<ante>`, editions use
  `standard_edition<ante>`, seals use `stdseal<ante>`, and seal type uses
  `stdsealtype<ante>`. Source: `card.lua` (`Card:open`).
- Erratic deck sampling uses repeated `pseudorandom_element(G.P_CARDS,
  pseudoseed('erratic'))` calls during the 52-iteration `P_CARDS` deck creation
  loop. Source: `game.lua` (`Game:start_run`).
- Immolate's Spectral effect sorts the current hand by `playing_card`, shuffles
  that temporary list with `pseudoshuffle(temp_hand, pseudoseed('immolate'))`,
  destroys the first `self.ability.extra.destroy` cards, and awards dollars.
  Source: `card.lua` (`Card:use_consumeable`, Immolate branch).

## Erratic Deck

- The Erratic deck sets `G.GAME.starting_params.erratic_suits_and_ranks = true`
  when the selected Back has `randomize_rank_suit`. Source: `back.lua`
  (`Back:apply_to_run`).
- During deck creation in `Game:start_run`, if
  `erratic_suits_and_ranks` is true, each iteration uses
  `pseudorandom_element(G.P_CARDS, pseudoseed('erratic'))` to pick a card key
  before applying deck restrictions and `no_faces`. The resulting `card_protos`
  are sorted, then `card_from_control()` creates the cards, and
  `starting_deck_size` is set to `#G.playing_cards`. Sources: `game.lua`
  (`Game:start_run`); `functions/misc_functions.lua` (`card_from_control`).
- For optimization and seed parity, Erratic sampling is a fixed 52 draws from
  the sorted `G.P_CARDS` key order. If `no_faces` is active, face samples are
  discarded after sampling; they are not replaced. Source: `game.lua`
  (`Game:start_run`).

## Modding Pitfalls and Practical Rules

- Prefer `Moveable`/`UIBox`/`Event` composition over direct LOVE calls for
  gameplay UI. This keeps controller locks, pause behavior, speed factor,
  alignment, draw hash, and save/transition cleanup coherent. Sources:
  `engine/moveable.lua`; `engine/ui.lua`; `engine/event.lua`; `game.lua`.
- Add behavior where runtime code dispatches it. New Back, Blind, Tag, Booster,
  and Joker data may appear in pools/UI but not fully behave unless relevant
  runtime branches or hooks exist. Sources: `back.lua`; `blind.lua`; `tag.lua`;
  `card.lua`.
- Do not rename runtime English `name` fields unless you have audited every
  branch. Use localization files for display names. Sources: `card.lua`;
  `back.lua`; `blind.lua`; `tag.lua`; `functions/common_events.lua`.
- Keep custom save state serializable. Save plain IDs/flags/tables and rebuild
  runtime objects through constructors or source helpers on load. Sources:
  `engine/string_packer.lua`; `functions/misc_functions.lua` (`save_run`);
  `cardarea.lua` (`CardArea:load`).
- Call `:remove()` on owned UIBoxes, Cards, CardAreas, particles, and child
  objects when they leave the stage; many instance registries are manually
  maintained. Sources: `engine/node.lua`; `card.lua`; `cardarea.lua`;
  `engine/ui.lua`.
- UI callbacks should live in `G.FUNCS` and be wired through `config.button` or
  `config.func`; controller navigation should use `focus_args`. Sources:
  `engine/ui.lua`; `functions/UI_definitions.lua`; `functions/button_callbacks.lua`.
- Respect pool placeholders for deterministic logic. Do not turn a filtered pool
  into a compact array unless exact RNG parity is irrelevant or you implement
  equivalent resampling. Source: `functions/common_events.lua`
  (`get_current_pool`).
- `banned_keys` is a flat key set used across cards, boosters, vouchers, tags,
  and challenge restrictions. Do not assume the source preserves restriction
  categories at runtime. Sources: `game.lua` (`Game:start_run`);
  `functions/common_events.lua`; `challenges.lua`.
- Seeded and challenge runs skip most unlock/discovery/stat progression. Do not
  test progression logic only under seeded runs. Sources:
  `functions/common_events.lua` (`unlock_card`, `discover_card`);
  `functions/state_events.lua` (`win_game`).

## Native Model Boundaries

- The Rust native search is the implementation for Brainstorm's shipped DLL
  behavior. `BalatroSource/` is the source of truth for game mechanics, but some
  native simplifications are intentional or historical.
- Existing native search operates at family level for booster packs, ignores
  exact concrete booster variants, scans two shop Joker slots even when source
  shop size could be larger, and treats `observatory` as a Telescope plus Mega
  Celestial availability filter.
- Observatory does not alter pack generation. The voucher's gameplay effect is
  checked when a held Planet card matches the scoring hand. Current Immolate's
  `observatory` filter is a search shortcut for "ante-1 Telescope voucher plus
  a Mega Celestial pack available", not a simulation of the Observatory voucher
  itself. Sources: `card.lua` (`Card:open`); `functions/state_events.lua`
  (`G.FUNCS.evaluate_play`); current Rust implementation.
- Perkeo's own Joker effect later copies a random consumable using the `perkeo`
  RNG key. Immolate's Perkeo search only needs the Soul-to-legendary path that
  yields Perkeo; it does not need to simulate Perkeo's later copying effect.
  Sources: `card.lua` (`Card:use_consumeable`, `Card:calculate_joker`);
  current Rust implementation.
- Native first-shop filters reject impossible requests before scanning. This
  includes ante-1 locked tags, voucher upgrades without their prerequisite
  voucher, Observatory with a non-Telescope voucher target, Observatory on
  Nebula Deck where Telescope is already active, Soul counts above one, pack
  Joker searches constrained to non-Buffoon packs, and Erratic requirements that
  cannot fit a 52-card opening deck. Sources: `game.lua` (`Game:start_run`);
  `back.lua` (`Back:apply_to_run`); `functions/common_events.lua`
  (`get_current_pool`, `get_next_voucher_key`, `get_pack`, `create_card`);
  current Rust implementation.
- Direct Joker search only targets Jokers that can appear in first-shop shop
  slots or Buffoon packs. Legendary/Soul-only Jokers, `enhancement_gate` Jokers,
  `yes_pool_flag` Jokers, and source-locked first-shop pool targets are hidden
  from the Lua selector and rejected by native target-pool classification.
  Sources: `game.lua` (`P_CENTERS`, `P_JOKER_RARITY_POOLS`);
  `functions/common_events.lua` (`get_current_pool`, `create_card`);
  current Rust implementation.
- Future optimized native implementations should preserve the current shipped
  Rust behavior by default, and only switch to source-correct expanded behavior
  behind explicit tests and UI contract changes.
