-- Brainstorm Supercharged Mod for Balatro
-- High-performance seed filtering and save state management
-- Full rewrite by KRVH. Originals: Brainstorm by OceanRamen; Immolate by MathIsFun0.
-- License: CC BY-NC-SA 4.0
-- Entry point loaded by lovely.toml; initializes config, UI, and game hooks.

local lovely = require("lovely")
local ffi = require("ffi")
local G = G

_G.Brainstorm = _G.Brainstorm or {}
local Brainstorm = _G.Brainstorm

Brainstorm.SPF_KEYS = {
  "1000",
  "2500",
  "5000",
  "10000",
  "25000",
  "50000",
  "100000",
  "250000",
  "500000",
  "1000000",
}

Brainstorm.SPF_LIST = {
  ["1000"] = 1000,
  ["2500"] = 2500,
  ["5000"] = 5000,
  ["10000"] = 10000,
  ["25000"] = 25000,
  ["50000"] = 50000,
  ["100000"] = 100000,
  ["250000"] = 250000,
  ["500000"] = 500000,
  ["1000000"] = 1000000,
}

Brainstorm.DEFAULT_SPF_KEY = "100000"

local DEFAULT_CONFIG = {
  enable = true,
  keybinds = {
    options = "t",
    modifier = "lctrl",
    f_reroll = "r",
    a_reroll = "a",
    save_state = "z",
    load_state = "x",
  },
  ar_filters = {
    pack = "",
    voucher_name = "",
    tag_name = "",
    tag2_name = "",
    joker_name = "",
    joker_search = "",
    joker_location = "any",
    soul_skip = 0,
    inst_observatory = false,
    inst_perkeo = false,
  },
  ar_prefs = {
    spf_int = 100000,
    face_count = 0,
    suit_ratio_percent = "Disabled",
  },
}

local function clone_table(value)
  if type(value) ~= "table" then
    return value
  end
  local out = {}
  for key, child in pairs(value) do
    out[key] = clone_table(child)
  end
  return out
end

function Brainstorm.default_config()
  return clone_table(DEFAULT_CONFIG)
end

Brainstorm.config = Brainstorm.config or Brainstorm.default_config()

Brainstorm.ar_active = Brainstorm.ar_active or false
Brainstorm.ar_seeds_scanned = Brainstorm.ar_seeds_scanned or 0
Brainstorm.ar_status_text = Brainstorm.ar_status_text or "Rerolling..."
Brainstorm.ar_status_last_update = Brainstorm.ar_status_last_update or 0
Brainstorm.ar_status_last_scanned = Brainstorm.ar_status_last_scanned or -1

Brainstorm.RATIO_MAP = {
  ["Disabled"] = 0,
  ["50%"] = 0.5,
  ["60%"] = 0.6,
  ["65%"] = 0.65,
  ["70%"] = 0.7,
  ["75%"] = 0.75,
  ["80%"] = 0.8,
  ["85%"] = 0.85,
}

Brainstorm.AR_STATUS_INTERVAL = 0.15

local pcall = pcall
local get_time = love and love.timer and love.timer.getTime

local DIRECTORY_NAME = "Brainstorm"
local CONFIG_FILE_NAME = "config.lua"
local UI_MODULE = "brainstorm_supercharged_ui"

local SEED_X_FACTOR = 0.33411983
local SEED_Y_FACTOR = 0.874146
local SEED_TIME_FACTOR = 0.412311010

local function build_seed_start()
  local seed_input = get_time() * SEED_TIME_FACTOR
  local hover = G.CONTROLLER and G.CONTROLLER.cursor_hover
  if hover and hover.T then
    seed_input = seed_input
      + hover.T.x * SEED_X_FACTOR
      + hover.T.y * SEED_Y_FACTOR
      + SEED_TIME_FACTOR * hover.time
  end
  if type(random_string) ~= "function" then
    return nil
  end
  return random_string(8, seed_input)
end

local function as_string(value)
  if type(value) == "string" then
    return value
  end
  if value == nil then
    return ""
  end
  return tostring(value)
end

local function as_number(value, default)
  local num = tonumber(value)
  if num == nil then
    return default
  end
  return num
end

local function as_int(value, default)
  local num = tonumber(value)
  if num == nil then
    return default
  end
  if num < 0 then
    return 0
  end
  return math.floor(num)
end

local function as_bool(value)
  return not not value
end

local function clamp_int(value, default, min_value, max_value)
  local num = math.floor(as_number(value, default))
  if num < min_value then
    return min_value
  end
  if max_value and num > max_value then
    return max_value
  end
  return num
end

local function is_run_stage()
  return G and G.STAGES and G.STAGE == G.STAGES.RUN and G.GAME ~= nil
end

local function attention_anchor()
  if is_run_stage() then
    return G.play
  end
  return G and G.title_top or nil
end

local function can_create_uibox(anchor)
  return G
    and G.E_MANAGER
    and G.UIT
    and G.STAGE
    and G.STAGE_OBJECTS
    and G.STAGE_OBJECTS[G.STAGE]
    and anchor ~= nil
end

function Brainstorm.is_enabled()
  return Brainstorm.config and Brainstorm.config.enable ~= false
end

local function assign_table(target, source)
  for key in pairs(target) do
    target[key] = nil
  end
  for key, value in pairs(source) do
    target[key] = value
  end
end

local function normalize_spf(prefs)
  local spf_int = math.floor(as_number(prefs.spf_int, 0))
  local spf_key = tostring(spf_int)
  if Brainstorm.SPF_LIST[spf_key] then
    prefs.spf_int = Brainstorm.SPF_LIST[spf_key]
    return
  end
  prefs.spf_int = Brainstorm.SPF_LIST[Brainstorm.DEFAULT_SPF_KEY]
end

local function merge_string(target, source, key)
  if type(source[key]) == "string" then
    target[key] = source[key]
  end
end

local function merge_bool(target, source, key)
  if type(source[key]) == "boolean" then
    target[key] = source[key]
  end
end

local function merge_int(target, source, key, min_value, max_value)
  if source[key] ~= nil then
    target[key] = clamp_int(source[key], target[key], min_value, max_value)
  end
end

function Brainstorm.normalize_config(source)
  local normalized = Brainstorm.default_config()
  if type(source) ~= "table" then
    return normalized
  end

  merge_bool(normalized, source, "enable")

  if type(source.keybinds) == "table" then
    for key in pairs(normalized.keybinds) do
      merge_string(normalized.keybinds, source.keybinds, key)
    end
  end

  local filters = source.ar_filters
  if type(filters) == "table" then
    merge_string(normalized.ar_filters, filters, "pack")
    merge_string(normalized.ar_filters, filters, "voucher_name")
    merge_string(normalized.ar_filters, filters, "tag_name")
    merge_string(normalized.ar_filters, filters, "tag2_name")
    merge_string(normalized.ar_filters, filters, "joker_name")
    merge_string(normalized.ar_filters, filters, "joker_search")
    merge_string(normalized.ar_filters, filters, "joker_location")
    merge_int(normalized.ar_filters, filters, "soul_skip", 0, 5)
    merge_bool(normalized.ar_filters, filters, "inst_observatory")
    merge_bool(normalized.ar_filters, filters, "inst_perkeo")
    if
      normalized.ar_filters.joker_location ~= "shop"
      and normalized.ar_filters.joker_location ~= "pack"
    then
      normalized.ar_filters.joker_location = "any"
    end
  end

  local prefs = source.ar_prefs
  if type(prefs) == "table" then
    merge_int(normalized.ar_prefs, prefs, "spf_int", 1)
    merge_int(normalized.ar_prefs, prefs, "face_count", 0, 35)
    merge_string(normalized.ar_prefs, prefs, "suit_ratio_percent")
  end
  normalize_spf(normalized.ar_prefs)

  if not Brainstorm.RATIO_MAP[normalized.ar_prefs.suit_ratio_percent] then
    normalized.ar_prefs.suit_ratio_percent = "Disabled"
  end

  return normalized
end

function Brainstorm.reset_config()
  assign_table(Brainstorm.config, Brainstorm.default_config())
  return Brainstorm.config
end

local function format_count(value)
  if type(number_format) == "function" then
    return number_format(value or 0)
  end
  return tostring(value or 0)
end

local function current_time()
  if type(get_time) == "function" then
    local success, now = pcall(get_time)
    if success and type(now) == "number" then
      return now
    end
  end
  return os.clock()
end

local function update_auto_reroll_status(force)
  local scanned = Brainstorm.ar_seeds_scanned or 0
  local now = current_time()
  if not force then
    if Brainstorm.ar_status_last_scanned == scanned then
      return
    end
    local interval = Brainstorm.AR_STATUS_INTERVAL or 0.15
    if now - (Brainstorm.ar_status_last_update or 0) < interval then
      return
    end
  end
  Brainstorm.ar_status_last_update = now
  Brainstorm.ar_status_last_scanned = scanned
  Brainstorm.ar_status_text = "Rerolling... scanned "
    .. format_count(scanned)
    .. " seeds"
end

local function current_seed_budget(prefs)
  if not prefs then
    return nil
  end

  local seed_budget = Brainstorm.SPF_LIST[tostring(as_int(prefs.spf_int, 0))]
  if seed_budget then
    return seed_budget
  end
  normalize_spf(prefs)
  return prefs.spf_int
end

function Brainstorm.config_path()
  if Brainstorm.CONFIG_PATH then
    return Brainstorm.CONFIG_PATH
  end

  if not (love and love.filesystem and love.filesystem.createDirectory) then
    return nil
  end
  local success, created =
    pcall(love.filesystem.createDirectory, DIRECTORY_NAME)
  if success and created then
    Brainstorm.CONFIG_PATH = DIRECTORY_NAME .. "/" .. CONFIG_FILE_NAME
    return Brainstorm.CONFIG_PATH
  end

  return nil
end

local create_status_text
local remove_status_text

local function show_auto_reroll_text()
  if Brainstorm.ar_text then
    return
  end
  local major = attention_anchor()
  if not can_create_uibox(major) then
    return
  end
  if not Brainstorm.ar_status_text or Brainstorm.ar_status_text == "" then
    update_auto_reroll_status(true)
  end
  Brainstorm.ar_text = create_status_text(major)
end

local function report_auto_reroll_error(message)
  if not message or message == Brainstorm.ar_last_error then
    return
  end
  Brainstorm.ar_last_error = message
  Brainstorm.save_state_alert(message)
end

function Brainstorm.load_config()
  local config_path = Brainstorm.config_path()
  local config_file
  if config_path then
    local read_success, contents = pcall(love.filesystem.read, config_path)
    if read_success then
      config_file = contents
    end
  end

  if not config_file then
    assign_table(
      Brainstorm.config,
      Brainstorm.normalize_config(Brainstorm.config)
    )
    Brainstorm.write_config()
  else
    local success, loaded_config = pcall(STR_UNPACK, config_file)
    if success then
      assign_table(
        Brainstorm.config,
        Brainstorm.normalize_config(loaded_config)
      )
    else
      assign_table(
        Brainstorm.config,
        Brainstorm.normalize_config(Brainstorm.config)
      )
    end
  end
end

function Brainstorm.write_config()
  local config_path = Brainstorm.config_path()
  if not config_path then
    return
  end
  local success, packed = pcall(STR_PACK, Brainstorm.config)
  if success and packed then
    pcall(love.filesystem.write, config_path, packed)
  end
end

function Brainstorm.init()
  if type(lovely.mod_dir) ~= "string" or lovely.mod_dir == "" then
    return false
  end
  Brainstorm.PATH = lovely.mod_dir .. "/" .. DIRECTORY_NAME
  Brainstorm.load_config()
  local success = pcall(require, UI_MODULE)
  if not success then
    package.loaded[UI_MODULE] = nil
  end
  return success
end

local save_state_keys = { "1", "2", "3", "4", "5" }

function Brainstorm.save_state_alert(text)
  local major = attention_anchor()
  if not can_create_uibox(major) then
    return
  end
  G.E_MANAGER:add_event(Event({
    trigger = "after",
    delay = 0.4,
    func = function()
      if not can_create_uibox(major) then
        return true
      end
      attention_text({
        text = text,
        scale = 0.7,
        hold = 3,
        major = major,
        backdrop_colour = G.C.SECONDARY_SET.Tarot,
        align = "cm",
        offset = { x = 0, y = -3.5 },
        silent = true,
      })
      G.E_MANAGER:add_event(Event({
        trigger = "after",
        delay = 0.06 * G.SETTINGS.GAMESPEED,
        blockable = false,
        blocking = false,
        func = function()
          play_sound("other1", 0.76, 0.4)
          return true
        end,
      }))
      return true
    end,
  }))
end

function Brainstorm.save_game_state(slot)
  if Brainstorm.is_enabled() and is_run_stage() then
    local save_path = G.SETTINGS.profile
      .. "/"
      .. "save_state_"
      .. slot
      .. ".jkr"
    local success = pcall(compress_and_save, save_path, G.ARGS.save_run)
    if success then
      Brainstorm.save_state_alert("Saved state to slot [" .. slot .. "]")
      return true
    else
      Brainstorm.save_state_alert("Failed to save state")
      return false
    end
  end
  return false
end

function Brainstorm.load_game_state(slot)
  if not Brainstorm.is_enabled() or not is_run_stage() then
    return false
  end
  local save_path = G.SETTINGS.profile .. "/" .. "save_state_" .. slot .. ".jkr"
  local success, saved_game = pcall(get_compressed, save_path)

  if success and saved_game then
    local unpack_success, saved_data = pcall(STR_UNPACK, saved_game)
    if unpack_success and saved_data then
      G:delete_run()
      G.SAVED_GAME = saved_data
      G:start_run({ savetext = G.SAVED_GAME })
      Brainstorm.save_state_alert("Loaded state from slot [" .. slot .. "]")
      return true
    else
      Brainstorm.save_state_alert("Corrupted save in slot [" .. slot .. "]")
      return false
    end
  else
    Brainstorm.save_state_alert("No save in slot [" .. slot .. "]")
    return false
  end
end

function Brainstorm.reroll()
  if not Brainstorm.is_enabled() or not is_run_stage() then
    return false
  end

  G.GAME.viewed_back = nil
  G.run_setup_seed = G.GAME.seeded
  G.challenge_tab = G.GAME and G.GAME.challenge and G.GAME.challenge_tab or nil
  G.forced_seed = G.GAME.seeded and G.GAME.pseudorandom.seed or nil

  local seed = G.run_setup_seed and G.setup_seed or G.forced_seed
  local stake = G.GAME.stake or G.PROFILES[G.SETTINGS.profile].MEMORY.stake or 1

  G:delete_run()
  G:start_run({ stake = stake, seed = seed, challenge = G.challenge_tab })
  return true
end

local function handle_brainstorm_keypress(key)
  if
    not Brainstorm.config
    or not Brainstorm.config.keybinds
    or not Brainstorm.is_enabled()
  then
    if Brainstorm.ar_active then
      Brainstorm.stop_auto_reroll()
    end
    return
  end

  if not is_run_stage() then
    return
  end

  local keybinds = Brainstorm.config.keybinds
  for _, slot in ipairs(save_state_keys) do
    if key == slot then
      if love.keyboard.isDown(keybinds.save_state) then
        Brainstorm.save_game_state(slot)
      end
      if love.keyboard.isDown(keybinds.load_state) then
        Brainstorm.load_game_state(slot)
      end
      break
    end
  end

  if love.keyboard.isDown(keybinds.modifier) then
    if key == keybinds.f_reroll then
      Brainstorm.reroll()
    elseif key == keybinds.a_reroll then
      local success = pcall(function()
        if Brainstorm.ar_active then
          Brainstorm.stop_auto_reroll()
        else
          Brainstorm.ar_active = true
          Brainstorm.ar_last_error = nil
          Brainstorm.ar_seeds_scanned = 0
          Brainstorm.ar_status_last_update = 0
          Brainstorm.ar_status_last_scanned = -1
          update_auto_reroll_status(true)
          show_auto_reroll_text()
        end
      end)
      if not success then
        Brainstorm.stop_auto_reroll()
      end
    end
  end
end

local function run_auto_reroll_update()
  local seed_found, err = Brainstorm.auto_reroll()
  if seed_found == false then
    Brainstorm.stop_auto_reroll()
    report_auto_reroll_error(err)
  elseif seed_found then
    local stake = G.GAME.stake
    local challenge = G.GAME and G.GAME.challenge and G.GAME.challenge_tab
    G:delete_run()
    G:start_run({
      stake = stake,
      seed = seed_found,
      challenge = challenge,
    })
    G.GAME.used_filter = true
    G.GAME.seeded = false
    Brainstorm.stop_auto_reroll()
  end
  if Brainstorm.ar_active then
    show_auto_reroll_text()
  end
end

local function update_auto_reroll()
  if not Brainstorm.ar_active then
    return
  end
  if not Brainstorm.is_enabled() or not is_run_stage() then
    Brainstorm.stop_auto_reroll()
    return
  end

  local success = pcall(run_auto_reroll_update)
  if not success then
    Brainstorm.stop_auto_reroll()
  end
end

Brainstorm._hooks = Brainstorm._hooks or {}

if not Brainstorm._hooks.key_press_update then
  Brainstorm._hooks.key_press_update = Controller.key_press_update
  function Controller:key_press_update(key, dt)
    Brainstorm._hooks.key_press_update(self, key, dt)
    handle_brainstorm_keypress(key)
  end
end

if not Brainstorm._hooks.game_update then
  Brainstorm._hooks.game_update = Game.update
  function Game:update(dt)
    if Brainstorm._hooks.game_update then
      Brainstorm._hooks.game_update(self, dt)
    end
    update_auto_reroll()
  end
end

local ffi_loaded = false
local native_handle = nil
local DLL_NAME = "Immolate.dll"

local function native_exports(handle)
  return handle.brainstorm_search, handle.free_result
end

local function init_ffi()
  if not ffi_loaded then
    local success = pcall(
      ffi.cdef,
      [[
      char* brainstorm_search(const char* seed_start, const char* voucher_key, const char* pack_key, const char* tag1_key, const char* tag2_key, const char* joker_name, const char* joker_location, double souls, bool observatory, bool perkeo, const char* deck_key, bool erratic, bool no_faces, int min_face_cards, double suit_ratio, long long num_seeds, int threads);
      void free_result(char* result);
    ]]
    )
    if not success then
      return false
    end
    ffi_loaded = true
  end
  return true
end

local function load_native()
  if native_handle then
    return native_handle
  end

  local dll_path = Brainstorm.PATH .. "/" .. DLL_NAME
  local success, handle = pcall(ffi.load, dll_path)
  if not success then
    return nil
  end

  local exports_success, search_fn, free_fn = pcall(native_exports, handle)
  if not exports_success or not search_fn or not free_fn then
    return nil
  end

  native_handle = {
    dll = handle,
    brainstorm_search = search_fn,
    free_result = free_fn,
  }
  return native_handle
end

function Brainstorm.stop_auto_reroll()
  Brainstorm.ar_active = false

  if Brainstorm.ar_text then
    Brainstorm.ar_text.cancelled = true
    if Brainstorm.ar_text.AT and not Brainstorm.ar_text.removing then
      remove_status_text(Brainstorm.ar_text)
    end
    Brainstorm.ar_text = nil
  end
end

function Brainstorm.auto_reroll()
  if not Brainstorm.is_enabled() or not is_run_stage() then
    return false, "Auto-reroll stopped (run unavailable)"
  end

  local config = Brainstorm.config
  local filters = config.ar_filters
  local prefs = config.ar_prefs

  local seed_start = build_seed_start()
  if not seed_start then
    return false, "Auto-reroll stopped (seed generator unavailable)"
  end

  if not init_ffi() then
    return false, "Auto-reroll stopped (FFI init failed)"
  end

  local immolate = load_native()
  if not immolate then
    return false, "Auto-reroll stopped (Immolate.dll missing)"
  end

  local pack_key = as_string(filters.pack)
  local selected_back = G.GAME and G.GAME.selected_back_key
  local deck_key = selected_back and as_string(selected_back.key) or ""

  local starting_params = G.GAME and G.GAME.starting_params
  local erratic = starting_params and starting_params.erratic_suits_and_ranks
    or false
  local no_faces = starting_params and starting_params.no_faces or false

  local min_face_cards = as_int(prefs.face_count, 0)
  local suit_ratio = Brainstorm.RATIO_MAP[prefs.suit_ratio_percent] or 0
  local seed_budget = current_seed_budget(prefs)
  if not seed_budget then
    return false, "Auto-reroll stopped (invalid seed budget)"
  end

  local call_success, result = pcall(
    immolate.brainstorm_search,
    seed_start,
    as_string(filters.voucher_name),
    pack_key,
    as_string(filters.tag_name),
    as_string(filters.tag2_name),
    as_string(filters.joker_name),
    as_string(filters.joker_location),
    as_number(filters.soul_skip, 0),
    as_bool(filters.inst_observatory),
    as_bool(filters.inst_perkeo),
    deck_key,
    as_bool(erratic),
    as_bool(no_faces),
    min_face_cards,
    suit_ratio,
    seed_budget,
    0
  )
  if not call_success then
    return false,
      "Auto-reroll stopped (native call failed: " .. tostring(result) .. ")"
  end

  if result == nil or result == ffi.NULL then
    Brainstorm.ar_seeds_scanned = (Brainstorm.ar_seeds_scanned or 0)
      + math.max(0, seed_budget)
    update_auto_reroll_status(false)
    return nil
  end

  local string_success, seed_found = pcall(ffi.string, result)
  local free_success, free_err = pcall(immolate.free_result, result)
  if not free_success then
    return false,
      "Auto-reroll stopped (native cleanup failed: "
        .. tostring(free_err)
        .. ")"
  end
  if not string_success then
    return false,
      "Auto-reroll stopped (native result failed: "
        .. tostring(seed_found)
        .. ")"
  end

  return seed_found
end

if not Brainstorm._hooks.round_scores_row then
  Brainstorm._hooks.round_scores_row = create_UIBox_round_scores_row
  function create_UIBox_round_scores_row(score, text_colour)
    local ret = Brainstorm._hooks.round_scores_row(score, text_colour)
    if not Brainstorm.is_enabled() then
      return ret
    end
    local seed_node = ret
      and ret.nodes
      and ret.nodes[2]
      and ret.nodes[2].nodes
      and ret.nodes[2].nodes[1]
    if seed_node and seed_node.config and score == "seed" and G and G.GAME then
      seed_node.config.colour = G.GAME.seeded and G.C.RED
        or G.GAME.used_filter and G.C.BLUE
        or G.C.BLACK
    end
    return ret
  end
end

create_status_text = function(major)
  if not (can_create_uibox(major) and G.C and copy_table) then
    return nil
  end
  local status = {
    colour = copy_table(G.C.WHITE),
    fade = 1,
  }
  G.E_MANAGER:add_event(Event({
    trigger = "after",
    delay = 0,
    blockable = false,
    blocking = false,
    func = function()
      if status.cancelled or not can_create_uibox(major) then
        return true
      end
      status.AT = UIBox({
        T = { 0, 0, 0, 0 },
        definition = {
          n = G.UIT.ROOT,
          config = {
            align = "cm",
            minw = 0.001,
            minh = 0.001,
            padding = 0.03,
            r = 0.1,
            colour = copy_table(G.C.CLEAR),
          },
          nodes = {
            {
              n = G.UIT.T,
              config = {
                draw_layer = 1,
                ref_table = Brainstorm,
                ref_value = "ar_status_text",
                scale = 1.4,
                colour = status.colour,
                shadow = true,
              },
            },
          },
        },
        config = {
          align = "cm",
          offset = { x = 0, y = -3.5 },
          major = major,
        },
      })
      status.AT.attention_text = true
      status.text = status.AT.UIRoot.children[1]
      return true
    end,
  }))
  return status
end

remove_status_text = function(status)
  if not status or status.removing then
    return
  end
  if not status.AT or status.AT.REMOVED then
    status.AT = nil
    return
  end
  if not (G and G.E_MANAGER) then
    status.AT:remove()
    status.AT = nil
    return
  end

  status.removing = true
  G.E_MANAGER:add_event(Event({
    trigger = "after",
    delay = 0,
    blockable = false,
    blocking = false,
    func = function()
      if not status.AT or status.AT.REMOVED then
        status.AT = nil
        status.removing = false
        return true
      end
      if not status.start_time then
        status.start_time = G.TIMERS.TOTAL
        if status.text and status.text.pop_out then
          status.text:pop_out(2)
        end
      else
        status.fade = math.max(0, 1 - 3 * (G.TIMERS.TOTAL - status.start_time))
        status.colour[4] = math.min(status.colour[4], status.fade)
        if status.fade <= 0 then
          status.AT:remove()
          status.AT = nil
          status.removing = false
          return true
        end
      end
      return false
    end,
  }))
end
