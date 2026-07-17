-- Brainstorm Supercharged Mod for Balatro
-- High-performance seed filtering and save state management
-- Full rewrite by KRVH. Originals: Brainstorm by OceanRamen; Immolate by MathIsFun0.
-- License: CC BY-NC-SA 4.0
-- Entry point loaded by lovely.toml; initializes config, UI, and game hooks.

local lovely = require("lovely")
local nfs = require("nativefs")
local ffi = require("ffi")

_G.Brainstorm = _G.Brainstorm or {}
Brainstorm = _G.Brainstorm

Brainstorm.VERSION = Brainstorm.VERSION or "Brainstorm Supercharged"

-- Steamodded compatibility reserves Brainstorm.SMODS for reload-time state.

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
Brainstorm.DEFAULT_SPF_ID = 7

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
    pack = {},
    pack_id = 1,
    voucher_name = "",
    voucher_id = 1,
    tag_name = "",
    tag_id = 1,
    tag2_name = "",
    tag2_id = 1,
    joker_name = "",
    joker_search = "",
    joker_id = 1,
    joker_location = "any",
    joker_location_id = 1,
    soul_skip = 0,
    inst_observatory = false,
    inst_perkeo = false,
  },
  ar_prefs = {
    spf_id = 7,
    spf_int = 100000,
    face_count = 0,
    suit_ratio_id = 1,
    suit_ratio_percent = "Disabled",
    suit_ratio_decimal = 0,
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

Brainstorm.ar_timer = Brainstorm.ar_timer or 0
Brainstorm.ar_frames = Brainstorm.ar_frames or 0
Brainstorm.ar_text = Brainstorm.ar_text or nil
Brainstorm.ar_active = Brainstorm.ar_active or false
Brainstorm.ar_last_error = Brainstorm.ar_last_error or nil
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

Brainstorm.AR_INTERVAL = 0.01
Brainstorm.AR_STATUS_INTERVAL = 0.15

local string_lower = string.lower
local pcall = pcall
local get_time = love and love.timer and love.timer.getTime

local CONFIG_DIR_NAME = "Brainstorm"
local CONFIG_FILE_NAME = "config.lua"

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

local function load_version_info()
  if not Brainstorm.PATH then
    return
  end

  local lovely_content = nfs.read(Brainstorm.PATH .. "/lovely.toml")
  if not lovely_content then
    return
  end

  local version = lovely_content:match('version%s*=%s*"([^"]+)"')
  if version then
    Brainstorm.VERSION = "Brainstorm Supercharged v" .. version
  end
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
  return value and true or false
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
    target[key] = clone_table(value)
  end
end

local function option_index_for_value(options, value)
  for i, option in ipairs(options) do
    if option == value then
      return i
    end
  end
  return 1
end

local function normalize_spf(prefs)
  local spf_id =
    clamp_int(prefs.spf_id, Brainstorm.DEFAULT_SPF_ID, 1, #Brainstorm.SPF_KEYS)
  local spf_int = math.floor(as_number(prefs.spf_int, 0))
  local spf_key = tostring(spf_int)
  if Brainstorm.SPF_LIST[spf_key] then
    prefs.spf_int = Brainstorm.SPF_LIST[spf_key]
    prefs.spf_id = option_index_for_value(Brainstorm.SPF_KEYS, spf_key)
    return
  end
  local selected_key = Brainstorm.SPF_KEYS[spf_id] or Brainstorm.DEFAULT_SPF_KEY
  prefs.spf_id = option_index_for_value(Brainstorm.SPF_KEYS, selected_key)
  prefs.spf_int = Brainstorm.SPF_LIST[selected_key]
    or Brainstorm.SPF_LIST[Brainstorm.DEFAULT_SPF_KEY]
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
    if type(filters.pack) == "table" or type(filters.pack) == "string" then
      normalized.ar_filters.pack = clone_table(filters.pack)
    end
    merge_int(normalized.ar_filters, filters, "pack_id", 1)
    merge_string(normalized.ar_filters, filters, "voucher_name")
    merge_int(normalized.ar_filters, filters, "voucher_id", 1)
    merge_string(normalized.ar_filters, filters, "tag_name")
    merge_int(normalized.ar_filters, filters, "tag_id", 1)
    merge_string(normalized.ar_filters, filters, "tag2_name")
    merge_int(normalized.ar_filters, filters, "tag2_id", 1)
    merge_string(normalized.ar_filters, filters, "joker_name")
    merge_string(normalized.ar_filters, filters, "joker_search")
    merge_int(normalized.ar_filters, filters, "joker_id", 1)
    merge_string(normalized.ar_filters, filters, "joker_location")
    merge_int(normalized.ar_filters, filters, "joker_location_id", 1)
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
    merge_int(normalized.ar_prefs, prefs, "spf_id", 1, #Brainstorm.SPF_KEYS)
    if prefs.spf_int ~= nil then
      normalized.ar_prefs.spf_int = prefs.spf_int
    end
    merge_int(normalized.ar_prefs, prefs, "face_count", 0, 35)
    merge_int(normalized.ar_prefs, prefs, "suit_ratio_id", 1)
    merge_string(normalized.ar_prefs, prefs, "suit_ratio_percent")
  end
  normalize_spf(normalized.ar_prefs)

  if not Brainstorm.RATIO_MAP[normalized.ar_prefs.suit_ratio_percent] then
    normalized.ar_prefs.suit_ratio_percent = "Disabled"
  end
  normalized.ar_prefs.suit_ratio_decimal = Brainstorm.RATIO_MAP[normalized.ar_prefs.suit_ratio_percent]
    or 0

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

local function current_seed_budget()
  local prefs = Brainstorm.config and Brainstorm.config.ar_prefs
  if not prefs then
    return nil
  end

  local seed_budget = as_int(prefs.spf_int, 0)
  if not Brainstorm.SPF_LIST[tostring(seed_budget)] then
    normalize_spf(prefs)
    seed_budget = as_int(prefs.spf_int, 0)
  end
  if Brainstorm.SPF_LIST[tostring(seed_budget)] then
    return seed_budget
  end
  return nil
end

local function file_exists(file_path)
  if type(file_path) ~= "string" or file_path == "" then
    return false
  end
  local success, info = pcall(nfs.getInfo, file_path)
  return success and info ~= nil
end

local function directory_exists(directory)
  if type(directory) ~= "string" or directory == "" then
    return false
  end
  local success, info = pcall(nfs.getInfo, directory, "directory")
  return success and info ~= nil
end

local function has_mod_markers(directory)
  return file_exists(directory .. "/lovely.toml")
    and file_exists(directory .. "/Brainstorm.lua")
    and file_exists(directory .. "/UI.lua")
end

local function get_save_directory()
  if not (love and love.filesystem and love.filesystem.getSaveDirectory) then
    return nil
  end
  local success, directory = pcall(love.filesystem.getSaveDirectory)
  if not success or type(directory) ~= "string" or directory == "" then
    return nil
  end
  return directory
end

function Brainstorm.config_path()
  if Brainstorm.CONFIG_PATH then
    return Brainstorm.CONFIG_PATH
  end

  local save_directory = get_save_directory()
  if save_directory then
    local config_directory = save_directory .. "/" .. CONFIG_DIR_NAME
    local create_success, created = pcall(nfs.createDirectory, config_directory)
    if (create_success and created) or directory_exists(config_directory) then
      Brainstorm.CONFIG_PATH = config_directory .. "/" .. CONFIG_FILE_NAME
      return Brainstorm.CONFIG_PATH
    end
  end

  return nil
end

local function legacy_config_path()
  if not Brainstorm.PATH then
    return nil
  end
  return Brainstorm.PATH .. "/" .. CONFIG_FILE_NAME
end

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
  Brainstorm.ar_text = Brainstorm.attention_text({
    scale = 1.4,
    ref_table = Brainstorm,
    ref_value = "ar_status_text",
    align = "cm",
    offset = { x = 0, y = -3.5 },
    major = major,
  })
end

local function report_auto_reroll_error(message)
  if not message or message == Brainstorm.ar_last_error then
    return
  end
  Brainstorm.ar_last_error = message
  Brainstorm.save_state_alert(message)
end

local function find_brainstorm_directory(directory)
  if type(directory) ~= "string" or directory == "" then
    return nil
  end
  local exact_path = directory .. "/Brainstorm"
  if directory_exists(exact_path) and has_mod_markers(exact_path) then
    return exact_path
  end

  local success, items = pcall(nfs.getDirectoryItems, directory)
  if not success or type(items) ~= "table" then
    return nil
  end
  table.sort(items)
  for _, item in ipairs(items) do
    local item_path = directory .. "/" .. item
    if directory_exists(item_path) then
      local item_name = string_lower(item)
      if item_name == "brainstorm" and has_mod_markers(item_path) then
        return item_path
      end
    end
  end
  return nil
end

function Brainstorm.load_config()
  local config_path = Brainstorm.config_path()
  local config_file = config_path
      and file_exists(config_path)
      and nfs.read(config_path)
    or nil
  local migrated = false

  local legacy_path = legacy_config_path()
  if not config_file and legacy_path and legacy_path ~= config_path then
    config_file = file_exists(legacy_path) and nfs.read(legacy_path) or nil
    migrated = config_file ~= nil
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
    if migrated then
      Brainstorm.write_config()
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
    local write_success = nfs.write(config_path, packed)
    if not write_success then
      return
    end
  end
end

function Brainstorm.init()
  Brainstorm.PATH = find_brainstorm_directory(lovely.mod_dir)
  if not Brainstorm.PATH then
    return false
  end
  load_version_info()

  Brainstorm.load_config()

  local ui_path = Brainstorm.PATH .. "/UI.lua"
  local ui_content = nfs.read(ui_path)
  if not ui_content then
    return false
  end

  local ui_func = load(ui_content)
  if not ui_func then
    return false
  end

  local success = pcall(ui_func)
  if not success then
    return false
  end

  return true
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
  local G = G
  if not Brainstorm.is_enabled() or not is_run_stage() then
    return false
  end

  G.GAME.viewed_back = nil
  G.run_setup_seed = G.GAME.seeded
  G.challenge_tab = G.GAME and G.GAME.challenge and G.GAME.challenge_tab or nil
  G.forced_seed = G.GAME.seeded and G.GAME.pseudorandom.seed or nil

  local seed = G.run_setup_seed and G.setup_seed or G.forced_seed
  local stake = (
    G.GAME.stake
    or G.PROFILES[G.SETTINGS.profile].MEMORY.stake
    or 1
  ) or 1

  G:delete_run()
  G:start_run({ stake = stake, seed = seed, challenge = G.challenge_tab })
  return true
end

local function handle_brainstorm_keypress(key)
  if
    not Brainstorm
    or not Brainstorm.config
    or not Brainstorm.config.keybinds
    or not Brainstorm.is_enabled()
  then
    if Brainstorm and Brainstorm.ar_active then
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

local function update_auto_reroll(dt)
  if not Brainstorm or not Brainstorm.ar_active then
    return
  end
  if not Brainstorm.is_enabled() or not is_run_stage() then
    Brainstorm.stop_auto_reroll()
    return
  end

  local success = pcall(function()
    Brainstorm.ar_frames = Brainstorm.ar_frames + 1
    Brainstorm.ar_timer = Brainstorm.ar_timer + dt

    if Brainstorm.ar_timer >= Brainstorm.AR_INTERVAL then
      Brainstorm.ar_timer = Brainstorm.ar_timer - Brainstorm.AR_INTERVAL
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
    end
    if Brainstorm.ar_active then
      show_auto_reroll_text()
    end
  end)
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
    update_auto_reroll(dt)
  end
end

local ffi_loaded = false
local native_handle = nil
local DLL_NAME = "Immolate.dll"

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
  local dll_file = io.open(dll_path, "rb")
  if not dll_file then
    return nil
  end
  dll_file:close()

  local success, handle = pcall(ffi.load, dll_path)
  if not success then
    return nil
  end

  local search_success, search_fn = pcall(function()
    return handle.brainstorm_search
  end)
  local free_success, free_fn = pcall(function()
    return handle.free_result
  end)
  if not search_success or not search_fn or not free_success or not free_fn then
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
  Brainstorm.ar_frames = 0

  if Brainstorm.ar_text then
    Brainstorm.ar_text.cancelled = true
    if Brainstorm.ar_text.AT and not Brainstorm.ar_text.removing then
      Brainstorm.remove_attention_text(Brainstorm.ar_text)
    end
    Brainstorm.ar_text = nil
  end
end

function Brainstorm.auto_reroll()
  if not Brainstorm.is_enabled() or not is_run_stage() then
    return false, "Auto-reroll stopped (run unavailable)"
  end

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

  local pack_key = ""
  local pack_filter = Brainstorm.config.ar_filters.pack
  if type(pack_filter) == "table" and #pack_filter > 0 then
    pack_key = pack_filter[1]
  elseif type(pack_filter) == "string" then
    pack_key = pack_filter
  end

  local deck_key = ""
  if G.GAME then
    local back_key = G.GAME.selected_back_key
    if type(back_key) == "string" then
      deck_key = back_key
    elseif type(back_key) == "table" and type(back_key.key) == "string" then
      deck_key = back_key.key
    elseif
      G.GAME.selected_back
      and G.GAME.selected_back.effect
      and G.GAME.selected_back.effect.center
      and type(G.GAME.selected_back.effect.center.key) == "string"
    then
      deck_key = G.GAME.selected_back.effect.center.key
    end
  end

  local erratic = G.GAME
      and G.GAME.starting_params
      and G.GAME.starting_params.erratic_suits_and_ranks
    or false
  local no_faces = G.GAME
      and G.GAME.starting_params
      and G.GAME.starting_params.no_faces
    or false

  local min_face_cards = as_int(Brainstorm.config.ar_prefs.face_count, 0)
  local suit_ratio = as_number(Brainstorm.config.ar_prefs.suit_ratio_decimal, 0)
  local seed_budget = current_seed_budget()
  if not seed_budget then
    return false, "Auto-reroll stopped (invalid seed budget)"
  end

  local call_success, result = pcall(
    immolate.brainstorm_search,
    seed_start,
    as_string(Brainstorm.config.ar_filters.voucher_name),
    as_string(pack_key),
    as_string(Brainstorm.config.ar_filters.tag_name),
    as_string(Brainstorm.config.ar_filters.tag2_name),
    as_string(Brainstorm.config.ar_filters.joker_name),
    as_string(Brainstorm.config.ar_filters.joker_location),
    as_number(Brainstorm.config.ar_filters.soul_skip, 0),
    as_bool(Brainstorm.config.ar_filters.inst_observatory),
    as_bool(Brainstorm.config.ar_filters.inst_perkeo),
    as_string(deck_key),
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

function Brainstorm.attention_text(args)
  args = args or {}
  if not (G and G.C and copy_table) then
    args.cancelled = true
    return args
  end
  args.text = args.text or "test"
  args.scale = args.scale or 1
  args.colour = copy_table(args.colour or G.C.WHITE)
  args.hold = (args.hold or 0) + 0.1 * (G.SPEEDFACTOR or 1)
  args.pos = args.pos or { x = 0, y = 0 }
  args.align = args.align or "cm"
  args.emboss = args.emboss or nil

  args.fade = 1
  local major = args.cover or args.major or attention_anchor()
  if not can_create_uibox(major) then
    args.cancelled = true
    return args
  end

  if args.cover then
    args.cover_colour = copy_table(args.cover_colour or G.C.RED)
    args.cover_colour_l = copy_table(lighten(args.cover_colour, 0.2))
    args.cover_colour_d = copy_table(darken(args.cover_colour, 0.2))
  else
    args.cover_colour = copy_table(G.C.CLEAR)
  end

  args.uibox_config = {
    align = args.align or "cm",
    offset = args.offset or { x = 0, y = 0 },
    major = major,
  }

  G.E_MANAGER:add_event(Event({
    trigger = "after",
    delay = 0,
    blockable = false,
    blocking = false,
    func = function()
      if args.cancelled or not can_create_uibox(major) then
        return true
      end
      args.AT = UIBox({
        T = { args.pos.x, args.pos.y, 0, 0 },
        definition = {
          n = G.UIT.ROOT,
          config = {
            align = args.cover_align or "cm",
            minw = (args.cover and args.cover.T.w or 0.001)
              + (args.cover_padding or 0),
            minh = (args.cover and args.cover.T.h or 0.001)
              + (args.cover_padding or 0),
            padding = 0.03,
            r = 0.1,
            emboss = args.emboss,
            colour = args.cover_colour,
          },
          nodes = {
            {
              n = G.UIT.T,
              config = {
                draw_layer = 1,
                text = args.text,
                ref_table = args.ref_table,
                ref_value = args.ref_value,
                scale = args.scale,
                colour = args.colour,
                shadow = true,
              },
            },
          },
        },
        config = args.uibox_config,
      })
      args.AT.attention_text = true

      args.text = args.AT.UIRoot.children[1]

      if args.cover then
        Particles(args.pos.x, args.pos.y, 0, 0, {
          timer_type = "TOTAL",
          timer = 0.01,
          pulse_max = 15,
          max = 0,
          scale = 0.3,
          vel_variation = 0.2,
          padding = 0.1,
          fill = true,
          lifespan = 0.5,
          speed = 2.5,
          attach = args.AT.UIRoot,
          colours = {
            args.cover_colour,
            args.cover_colour_l,
            args.cover_colour_d,
          },
        })
      end
      if args.backdrop_colour then
        args.backdrop_colour = copy_table(args.backdrop_colour)
        Particles(args.pos.x, args.pos.y, 0, 0, {
          timer_type = "TOTAL",
          timer = 5,
          scale = 2.4 * (args.backdrop_scale or 1),
          lifespan = 5,
          speed = 0,
          attach = args.AT,
          colours = { args.backdrop_colour },
        })
      end
      return true
    end,
  }))
  return args
end

function Brainstorm.remove_attention_text(args)
  if not args or args.removing then
    return
  end
  if not args.AT or args.AT.REMOVED then
    args.AT = nil
    return
  end
  if not (G and G.E_MANAGER) then
    args.AT:remove()
    args.AT = nil
    return
  end

  args.removing = true
  G.E_MANAGER:add_event(Event({
    trigger = "after",
    delay = 0,
    blockable = false,
    blocking = false,
    func = function()
      if not args.AT or args.AT.REMOVED then
        args.AT = nil
        args.removing = false
        return true
      end
      if not args.start_time then
        args.start_time = G.TIMERS.TOTAL
        if args.text and args.text.pop_out then
          args.text:pop_out(2)
        end
      else
        args.fade = math.max(0, 1 - 3 * (G.TIMERS.TOTAL - args.start_time))
        if args.cover_colour then
          args.cover_colour[4] = math.min(args.cover_colour[4], 2 * args.fade)
        end
        if args.cover_colour_l then
          args.cover_colour_l[4] = math.min(args.cover_colour_l[4], args.fade)
        end
        if args.cover_colour_d then
          args.cover_colour_d[4] = math.min(args.cover_colour_d[4], args.fade)
        end
        if args.backdrop_colour then
          args.backdrop_colour[4] = math.min(args.backdrop_colour[4], args.fade)
        end
        if args.colour then
          args.colour[4] = math.min(args.colour[4], args.fade)
        end
        if args.fade <= 0 and args.AT then
          args.AT:remove()
          args.AT = nil
          args.removing = false
          return true
        end
      end
      return false
    end,
  }))
end
