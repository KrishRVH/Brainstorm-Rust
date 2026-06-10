-- luacheck: ignore 131/love 131/Controller 131/create_UIBox_round_scores_row 131/Event 131/UIBox

local repo = assert(arg[1], "repository path is required")
local native_calls = {}
local cuda_settings = {}
local cuda_at_search = {}
local active_cuda_setting
local fail_cuda_setting = false
local omit_cuda_export = false
local ffi_declarations
local native_loads = 0
local native_handle_ref = setmetatable({}, { __mode = "v" })

package.preload.lovely = function()
  return { mod_dir = repo }
end
package.preload.ffi = function()
  return {
    NULL = {},
    cdef = function(declarations)
      ffi_declarations = declarations
    end,
    load = function(path)
      assert(path == repo .. "/Brainstorm/Immolate.dll")
      native_loads = native_loads + 1
      local handle = {
        brainstorm_search = function(...)
          assert(active_cuda_setting ~= nil, "CUDA setting must precede search")
          cuda_at_search[#cuda_at_search + 1] = active_cuda_setting
          native_calls[#native_calls + 1] = { ... }
          return nil
        end,
        free_result = function()
          error("nil native results must not be freed")
        end,
        immolate_set_cuda_enabled = function(enabled)
          if fail_cuda_setting then
            error("synthetic CUDA setting failure")
          end
          cuda_settings[#cuda_settings + 1] = enabled
          active_cuda_setting = enabled
        end,
      }
      if omit_cuda_export then
        handle.immolate_set_cuda_enabled = nil
      end
      native_handle_ref[1] = handle
      return handle
    end,
    string = tostring,
  }
end

local original_updates = 0
local started_run
local deleted_runs = 0
local queued_events = {}
local captured_uibox
local removed_boxes = 0
local created_directory
local written_config_path
local written_config
local ui_loads = 0

love = {
  timer = {
    getTime = function()
      return 1
    end,
  },
  keyboard = {
    isDown = function()
      return false
    end,
  },
  filesystem = {
    createDirectory = function(path)
      created_directory = path
      return true
    end,
    read = function()
      return nil
    end,
    write = function(path, contents)
      written_config_path = path
      written_config = contents
      return true
    end,
  },
}
Controller = { key_press_update = function() end }
Game = {
  update = function()
    original_updates = original_updates + 1
  end,
}
create_UIBox_round_scores_row = function()
  return { nodes = {} }
end

G = {
  STAGES = { RUN = "RUN" },
  STAGE = "RUN",
  STAGE_OBJECTS = { RUN = true },
  GAME = { stake = 2, seeded = true },
  play = { T = { x = 0, y = 0, w = 1, h = 1 } },
  title_top = {},
  UIT = { ROOT = "ROOT", T = "TEXT" },
  TIMERS = { TOTAL = 0 },
  C = {
    WHITE = { 1, 1, 1, 1 },
    CLEAR = { 0, 0, 0, 0 },
    RED = { 1, 0, 0, 1 },
    BLUE = { 0, 0, 1, 1 },
    BLACK = { 0, 0, 0, 1 },
  },
  E_MANAGER = {
    add_event = function(_, event)
      queued_events[#queued_events + 1] = event
    end,
  },
}
function G:delete_run()
  deleted_runs = deleted_runs + 1
end
function G:start_run(args)
  started_run = args
end

Event = function(args)
  return args
end
copy_table = function(value)
  local result = {}
  for key, child in pairs(value) do
    result[key] = child
  end
  return result
end
STR_PACK = function()
  return "packed config"
end
local seed_number = 0
random_string = function()
  seed_number = seed_number + 1
  return ("SEED%04d"):format(seed_number)
end
UIBox = function(args)
  captured_uibox = args
  local box = {
    UIRoot = { children = { {} } },
    REMOVED = false,
  }
  function box:remove()
    self.REMOVED = true
    removed_boxes = removed_boxes + 1
  end
  return box
end

assert(loadfile(repo .. "/Brainstorm.lua"))()
assert(Brainstorm.init() == false)
assert(Brainstorm.PATH == repo .. "/Brainstorm")
assert(Brainstorm.config_path() == "Brainstorm/config.lua")
assert(created_directory == "Brainstorm")
assert(written_config_path == "Brainstorm/config.lua")
assert(written_config == "packed config")

local config_identity = Brainstorm.config
Brainstorm.reset_config()
assert(Brainstorm.config == config_identity)
assert(Brainstorm.config.ar_prefs.use_cuda)

package.preload.brainstorm_supercharged_ui = function()
  error("synthetic UI module failure")
end
assert(Brainstorm.init() == false)

package.preload.brainstorm_supercharged_ui = function()
  ui_loads = ui_loads + 1
  return true
end
assert(Brainstorm.init())
assert(Brainstorm.init())
assert(ui_loads == 1)

written_config = nil
love.filesystem.read = function(path)
  assert(path == "Brainstorm/config.lua")
  return "existing config"
end
STR_UNPACK = function(packed)
  assert(packed == "existing config")
  return {
    enable = false,
    ar_filters = { pack = "p_arcana_normal_1" },
    ar_prefs = {
      spf_int = 250000,
      use_cuda = false,
      suit_ratio_percent = "75%",
    },
  }
end
Brainstorm.load_config()
assert(Brainstorm.config == config_identity)
assert(not Brainstorm.config.enable)
assert(Brainstorm.config.ar_filters.pack == "p_arcana_normal_1")
assert(Brainstorm.config.ar_prefs.spf_int == 250000)
assert(not Brainstorm.config.ar_prefs.use_cuda)
assert(Brainstorm.config.ar_prefs.suit_ratio_percent == "75%")
assert(written_config == nil)

love.filesystem.read = function()
  error("synthetic config read failure")
end
assert(pcall(Brainstorm.load_config))
love.filesystem.write = function()
  error("synthetic config write failure")
end
assert(pcall(Brainstorm.write_config))

local normalized = Brainstorm.normalize_config({
  ar_filters = {
    pack = "p_arcana_normal_1",
    pack_id = 9,
    voucher_name = "v_telescope",
    voucher_id = 7,
  },
  ar_prefs = {
    spf_int = 250000,
    spf_id = 2,
    use_cuda = false,
    suit_ratio_percent = "75%",
    suit_ratio_id = 6,
    suit_ratio_decimal = 0.1,
  },
})
assert(normalized.ar_filters.pack == "p_arcana_normal_1")
assert(normalized.ar_filters.voucher_name == "v_telescope")
assert(normalized.ar_prefs.spf_int == 250000)
assert(not normalized.ar_prefs.use_cuda)
assert(normalized.ar_prefs.suit_ratio_percent == "75%")
for _, key in ipairs({
  "pack_id",
  "voucher_id",
  "tag_id",
  "tag2_id",
  "joker_id",
  "joker_location_id",
}) do
  assert(normalized.ar_filters[key] == nil)
end
for _, key in ipairs({ "spf_id", "suit_ratio_id", "suit_ratio_decimal" }) do
  assert(normalized.ar_prefs[key] == nil)
end
assert(
  Brainstorm.normalize_config({ ar_filters = { pack = { "legacy" } } }).ar_filters.pack
    == ""
)
assert(
  Brainstorm.normalize_config({ ar_prefs = { use_cuda = "legacy" } }).ar_prefs.use_cuda
)
assert(
  Brainstorm.normalize_config({ ar_filters = { soul_skip = 5 } }).ar_filters.soul_skip
    == 1
)

local function assert_native_call(actual, expected)
  assert(#actual == 17)
  for index = 1, 17 do
    assert(
      actual[index] == expected[index],
      ("native argument %d: expected %s, got %s"):format(
        index,
        tostring(expected[index]),
        tostring(actual[index])
      )
    )
  end
end

local filters = Brainstorm.config.ar_filters
local prefs = Brainstorm.config.ar_prefs
Brainstorm.config.enable = true
filters.voucher_name = "v_telescope"
filters.pack = "p_arcana_normal_1"
filters.tag_name = "tag_charm"
filters.tag2_name = "tag_meteor"
filters.joker_name = "Blueprint"
filters.joker_location = "shop"
filters.soul_skip = 1
filters.inst_observatory = true
filters.inst_perkeo = false
prefs.face_count = 12
prefs.suit_ratio_percent = "75%"
prefs.spf_int = 100000
prefs.use_cuda = true
G.GAME = {
  stake = 2,
  seeded = true,
  selected_back_key = { key = "b_red" },
  starting_params = {},
}
omit_cuda_export = true
local missing_result, missing_error = Brainstorm.auto_reroll()
assert(
  missing_result == false
    and missing_error
      == "Auto-reroll stopped (Immolate.dll missing or incompatible)"
)
assert(#native_calls == 0 and #cuda_settings == 0)
omit_cuda_export = false
seed_number = 0
assert(Brainstorm.auto_reroll() == nil)

filters.voucher_name = "v_clearance_sale"
filters.pack = "p_spectral_mega_1"
filters.tag_name = "tag_buffoon"
filters.tag2_name = "tag_rare"
filters.joker_name = "Brainstorm"
filters.joker_location = "pack"
filters.soul_skip = 0
filters.inst_observatory = false
filters.inst_perkeo = true
prefs.face_count = 27
prefs.suit_ratio_percent = "85%"
prefs.spf_int = 250000
G.GAME.selected_back_key.key = "b_erratic"
G.GAME.starting_params.erratic_suits_and_ranks = true
G.GAME.starting_params.no_faces = true
assert(Brainstorm.auto_reroll() == nil)

assert(Brainstorm.config == config_identity)
assert(native_loads == 2 and #native_calls == 2)
assert(
  ffi_declarations:find(
    "void immolate_set_cuda_enabled(bool enabled);",
    1,
    true
  )
)
assert(#cuda_settings == 1 and cuda_settings[1] == true)
assert(cuda_at_search[1] == true and cuda_at_search[2] == true)
assert_native_call(native_calls[1], {
  "SEED0001",
  "v_telescope",
  "p_arcana_normal_1",
  "tag_charm",
  "tag_meteor",
  "Blueprint",
  "shop",
  1,
  true,
  false,
  "b_red",
  false,
  false,
  12,
  0.75,
  100000,
  0,
})
assert_native_call(native_calls[2], {
  "SEED0002",
  "v_clearance_sale",
  "p_spectral_mega_1",
  "tag_buffoon",
  "tag_rare",
  "Brainstorm",
  "pack",
  0,
  false,
  true,
  "b_erratic",
  true,
  true,
  27,
  0.85,
  250000,
  0,
})

prefs.use_cuda = false
assert(Brainstorm.auto_reroll() == nil)
assert(#native_calls == 3)
assert(#cuda_settings == 2 and cuda_settings[2] == false)
assert(cuda_at_search[3] == false)
assert(native_calls[3][1] == "SEED0003")
for index = 2, 17 do
  assert(native_calls[3][index] == native_calls[2][index])
end

fail_cuda_setting = true
prefs.use_cuda = true
local cuda_result, cuda_error = Brainstorm.auto_reroll()
assert(cuda_result == false and cuda_error:find("CUDA setting failed", 1, true))
assert(#native_calls == 3 and #cuda_settings == 2)

fail_cuda_setting = false
assert(Brainstorm.auto_reroll() == nil)
assert(#native_calls == 4)
assert(#cuda_settings == 3 and cuda_settings[3] == true)
assert(cuda_at_search[4] == true)
assert(native_calls[4][1] == "SEED0005")
for index = 2, 17 do
  assert(native_calls[4][index] == native_calls[3][index])
end
collectgarbage()
collectgarbage()
assert(native_handle_ref[1] ~= nil)

local function reset_active()
  Brainstorm.ar_active = true
  Brainstorm.ar_text = {}
  Brainstorm.ar_last_error = nil
end

for _, rate in ipairs({ 60, 144, 240 }) do
  local calls = 0
  Brainstorm.auto_reroll = function()
    calls = calls + 1
    return nil
  end
  reset_active()
  for _ = 1, rate do
    Game:update(1 / rate)
  end
  assert(calls == rate, ("expected %d searches, got %d"):format(rate, calls))
  Brainstorm.stop_auto_reroll()
end
assert(original_updates == 60 + 144 + 240)

Brainstorm.auto_reroll = function()
  return "ABC12345"
end
reset_active()
G.GAME = { stake = 2, seeded = true }
Game:update(1 / 60)
assert(not Brainstorm.ar_active)
assert(deleted_runs == 1)
assert(
  started_run and started_run.seed == "ABC12345" and started_run.stake == 2
)
assert(G.GAME.used_filter and not G.GAME.seeded)

local alert
Brainstorm.save_state_alert = function(message)
  alert = message
end
Brainstorm.auto_reroll = function()
  return false, "expected failure"
end
reset_active()
Game:update(1 / 60)
assert(not Brainstorm.ar_active and alert == "expected failure")

Brainstorm.auto_reroll = function()
  error("synthetic panic")
end
reset_active()
Game:update(1 / 60)
assert(not Brainstorm.ar_active)

Brainstorm.auto_reroll = function()
  error("search should not run outside a run")
end
reset_active()
G.STAGE = "MENU"
Game:update(1 / 60)
assert(not Brainstorm.ar_active)
G.STAGE = "RUN"

queued_events = {}
Brainstorm.auto_reroll = function()
  return nil
end
Brainstorm.ar_text = nil
Brainstorm.ar_active = true
Game:update(1 / 60)
local status = assert(Brainstorm.ar_text)
assert(#queued_events == 1 and not status.cancelled)
assert(queued_events[1].func())
assert(status.AT and captured_uibox)
local text_config = captured_uibox.definition.nodes[1].config
assert(captured_uibox.definition.nodes[1].n == G.UIT.T)
assert(text_config.ref_table == Brainstorm)
assert(text_config.ref_value == "ar_status_text")
assert(text_config.draw_layer == 1 and text_config.shadow == true)

Brainstorm.stop_auto_reroll()
assert(not Brainstorm.ar_active and Brainstorm.ar_text == nil)
assert(status.cancelled and status.removing and #queued_events == 2)
local removal = queued_events[2]
G.TIMERS.TOTAL = 0
assert(removal.func() == false)
G.TIMERS.TOTAL = 1
assert(removal.func() == true)
assert(status.AT == nil and not status.removing and removed_boxes == 1)

queued_events = {}
Brainstorm.ar_text = nil
Brainstorm.ar_active = true
Game:update(1 / 60)
local cancelled = assert(Brainstorm.ar_text)
assert(#queued_events == 1)
Brainstorm.stop_auto_reroll()
assert(cancelled.cancelled)
assert(queued_events[1].func())
assert(cancelled.AT == nil)

print("Lua lifecycle smoke: PASS")
