-- luacheck: globals Brainstorm G UIBox_button create_option_cycle create_tabs create_text_input create_toggle darken

local repo = assert(arg[1], "repository path is required")

Brainstorm = {
  DEFAULT_SPF_KEY = "100000",
  RATIO_MAP = {
    Disabled = 0,
    ["75%"] = 0.75,
  },
  SPF_KEYS = { "1000", "100000" },
  SPF_LIST = {
    ["1000"] = 1000,
    ["100000"] = 100000,
  },
  config = {
    enable = true,
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
  },
  write_config = function() end,
  reset_config = function() end,
}

G = {
  FUNCS = {},
  P_CENTER_POOLS = {
    Joker = {
      { set = "Joker", name = "Alpha", rarity = 1 },
      { set = "Joker", name = "Perkeo", rarity = 4 },
    },
  },
  UIT = {
    ROOT = "ROOT",
    C = "COLUMN",
    R = "ROW",
    T = "TEXT",
  },
  C = {
    CLEAR = {},
    BLUE = {},
    ORANGE = {},
    UI = {
      TRANSPARENT_DARK = {},
      TEXT_LIGHT = {},
    },
  },
}

local cycles = {}
create_option_cycle = function(args)
  cycles[args.label] = args
  return args
end
create_text_input = function(args)
  return args
end
create_toggle = function(args)
  return args
end
UIBox_button = function(args)
  return args
end
darken = function(colour)
  return colour
end

local original_tab_calls = 0
create_tabs = function(args)
  original_tab_calls = original_tab_calls + 1
  return args
end

assert(assert(loadfile(repo .. "/UI.lua"))() == true)

local args = { tab_h = 7.05, tabs = {} }
assert(create_tabs(args) == args)
assert(#args.tabs == 1 and original_tab_calls == 1)
assert(create_tabs(args) == args)
assert(#args.tabs == 1 and original_tab_calls == 2)

local settings_tab = args.tabs[1]
assert(settings_tab.label == "Brainstorm Supercharged")
assert(settings_tab.tab_definition_function())

local joker_options = cycles["AR: JOKER SEARCH"].options
assert(#joker_options == 2)
assert(joker_options[1] == "None" and joker_options[2] == "Alpha")

local face_options = cycles["ED: Min. # of Face Cards"].options
local soul_options = cycles["AR: N. SOULS"].options
assert(#face_options == 36 and face_options[1] == 0 and face_options[36] == 35)
assert(#soul_options == 2 and soul_options[1] == 0 and soul_options[2] == 1)

G.P_CENTER_POOLS.Joker[#G.P_CENTER_POOLS.Joker + 1] =
  { set = "Joker", name = "Beta", rarity = 2 }
assert(settings_tab.tab_definition_function())

local refreshed_jokers = cycles["AR: JOKER SEARCH"].options
assert(refreshed_jokers ~= joker_options)
assert(#refreshed_jokers == 3 and refreshed_jokers[3] == "Beta")
assert(cycles["ED: Min. # of Face Cards"].options == face_options)
assert(cycles["AR: N. SOULS"].options == soul_options)

print("Lua UI smoke: PASS")
