-- Brainstorm UI Module
-- Created by OceanRamen. Rewrite by KRVH. Immolate DLL by MathIsFun0.

local ipairs = ipairs
local pairs = pairs
local string_lower = string.lower

-- Note: "Speed Tag" is internally called "tag_skip" in Balatro
local tag_list = {
  ["None"] = "",
  ["Uncommon Tag"] = "tag_uncommon",
  ["Rare Tag"] = "tag_rare",
  ["Holographic Tag"] = "tag_holo",
  ["Foil Tag"] = "tag_foil",
  ["Polychrome Tag"] = "tag_polychrome",
  ["Investment Tag"] = "tag_investment",
  ["Voucher Tag"] = "tag_voucher",
  ["Boss Tag"] = "tag_boss",
  ["Charm Tag"] = "tag_charm",
  ["Juggle Tag"] = "tag_juggle",
  ["Double Tag"] = "tag_double",
  ["Coupon Tag"] = "tag_coupon",
  ["Economy Tag"] = "tag_economy",
  ["Speed Tag"] = "tag_skip",
  ["D6 Tag"] = "tag_d_six",
}

local voucher_list = {
  ["None"] = "",
  ["Overstock"] = "v_overstock_norm",
  ["Clearance Sale"] = "v_clearance_sale",
  ["Hone"] = "v_hone",
  ["Reroll Surplus"] = "v_reroll_surplus",
  ["Crystal Ball"] = "v_crystal_ball",
  ["Telescope"] = "v_telescope",
  ["Grabber"] = "v_grabber",
  ["Wasteful"] = "v_wasteful",
  ["Tarot Merchant"] = "v_tarot_merchant",
  ["Planet Merchant"] = "v_planet_merchant",
  ["Seed Money"] = "v_seed_money",
  ["Blank"] = "v_blank",
  ["Magic Trick"] = "v_magic_trick",
  ["Hieroglyph"] = "v_hieroglyph",
  ["Director's Cut"] = "v_directors_cut",
  ["Paint Brush"] = "v_paint_brush",
}

local pack_list = {
  ["None"] = {},
  ["Normal Arcana"] = {
    "p_arcana_normal_1",
    "p_arcana_normal_2",
    "p_arcana_normal_3",
    "p_arcana_normal_4",
  },
  ["Jumbo Arcana"] = { "p_arcana_jumbo_1", "p_arcana_jumbo_2" },
  ["Mega Arcana"] = { "p_arcana_mega_1", "p_arcana_mega_2" },
  ["Normal Celestial"] = {
    "p_celestial_normal_1",
    "p_celestial_normal_2",
    "p_celestial_normal_3",
    "p_celestial_normal_4",
  },
  ["Jumbo Celestial"] = { "p_celestial_jumbo_1", "p_celestial_jumbo_2" },
  ["Mega Celestial"] = { "p_celestial_mega_1", "p_celestial_mega_2" },
  ["Normal Standard"] = {
    "p_standard_normal_1",
    "p_standard_normal_2",
    "p_standard_normal_3",
    "p_standard_normal_4",
  },
  ["Jumbo Standard"] = { "p_standard_jumbo_1", "p_standard_jumbo_2" },
  ["Mega Standard"] = { "p_standard_mega_1", "p_standard_mega_2" },
  ["Normal Buffoon"] = { "p_buffoon_normal_1", "p_buffoon_normal_2" },
  ["Jumbo Buffoon"] = { "p_buffoon_jumbo_1" },
  ["Mega Buffoon"] = { "p_buffoon_mega_1" },
  ["Normal Spectral"] = { "p_spectral_normal_1", "p_spectral_normal_2" },
  ["Jumbo Spectral"] = { "p_spectral_jumbo_1" },
  ["Mega Spectral"] = { "p_spectral_mega_1" },
}

local function build_sorted_keys(list)
  local keys = {}
  for key in pairs(list) do
    keys[#keys + 1] = key
  end
  table.sort(keys, function(a, b)
    if a == b then
      return false
    end
    if a == "None" then
      return true
    end
    if b == "None" then
      return false
    end
    return a < b
  end)
  return keys
end

local tag_keys = build_sorted_keys(tag_list)
local voucher_keys = build_sorted_keys(voucher_list)
local pack_keys = build_sorted_keys(pack_list)

local joker_list = { ["None"] = "" }
local joker_keys = { "None" }

local joker_location_list = {
  ["In Any Location"] = "any",
  ["In Shop Slots"] = "shop",
  ["In Buffoon Packs"] = "pack",
}

local joker_location_keys = {
  "In Any Location",
  "In Shop Slots",
  "In Buffoon Packs",
}

local first_shop_impossible_joker_names = {
  ["Steel Joker"] = true,
  ["Stone Joker"] = true,
  ["Lucky Cat"] = true,
  ["Golden Ticket"] = true,
  ["Glass Joker"] = true,
  Cavendish = true,
  Caino = true,
  Canio = true,
  Triboulet = true,
  Yorick = true,
  Chicot = true,
  Perkeo = true,
}

local function is_searchable_joker_center(center)
  if not center or center.set ~= "Joker" or not center.name then
    return false
  end
  if
    center.rarity == 4
    or center.enhancement_gate
    or center.yes_pool_flag
    or first_shop_impossible_joker_names[center.name]
  then
    return false
  end
  return true
end

local function rebuild_joker_options()
  joker_list = { ["None"] = "" }
  joker_keys = { "None" }
  local pool = G and G.P_CENTER_POOLS and G.P_CENTER_POOLS.Joker
  if not pool then
    return
  end
  local search = ""
  if Brainstorm and Brainstorm.config and Brainstorm.config.ar_filters then
    search = Brainstorm.config.ar_filters.joker_search or ""
  end
  if type(search) ~= "string" then
    search = tostring(search or "")
  end
  search = string_lower(search)
  search = search:gsub("^%s+", ""):gsub("%s+$", "")
  local has_search = search ~= ""
  for _, center in ipairs(pool) do
    if
      is_searchable_joker_center(center) and joker_list[center.name] == nil
    then
      local name = center.name
      if (not has_search) or string_lower(name):find(search, 1, true) then
        joker_list[name] = name
        joker_keys[#joker_keys + 1] = name
      end
    end
  end
  table.sort(joker_keys, function(a, b)
    if a == b then
      return false
    end
    if a == "None" then
      return true
    end
    if b == "None" then
      return false
    end
    return a < b
  end)
end
local spf_list = Brainstorm.SPF_LIST
local spf_keys = Brainstorm.SPF_KEYS

local ratio_list = Brainstorm.RATIO_MAP

local ratio_keys =
  { "Disabled", "50%", "60%", "65%", "70%", "75%", "80%", "85%" }

local function clamp_index(index, max_value)
  if type(index) ~= "number" then
    return 1
  end
  index = math.floor(index)
  if index < 1 then
    return 1
  end
  if index > max_value then
    return max_value
  end
  return index
end

local function option_index_for_value(options, value)
  if value == nil or value == "" then
    return 1
  end
  for i, option in ipairs(options) do
    if option == value then
      return i
    end
  end
  return 1
end

local function option_index_for_mapping(options, mapping, value)
  if value == nil or value == "" then
    return 1
  end
  for i, option in ipairs(options) do
    if mapping[option] == value then
      return i
    end
  end
  return 1
end

local function pack_list_matches(a, b)
  if a == b then
    return true
  end
  if type(a) ~= "table" or type(b) ~= "table" then
    return false
  end
  if #a ~= #b then
    return false
  end
  for i = 1, #a do
    if a[i] ~= b[i] then
      return false
    end
  end
  return true
end

local function option_index_for_pack(options, pack_value)
  if pack_value == nil then
    return 1
  end
  if pack_value == "" then
    pack_value = {}
  end
  for i, option in ipairs(options) do
    local candidate = pack_list[option]
    if type(pack_value) == "table" then
      if pack_list_matches(candidate, pack_value) then
        return i
      end
    else
      if candidate == pack_value then
        return i
      end
      if type(candidate) == "table" then
        for j = 1, #candidate do
          if candidate[j] == pack_value then
            return i
          end
        end
      end
    end
  end
  return 1
end

local function location_index_for_value(value)
  if value == nil or value == "" then
    return 1
  end
  for i, option in ipairs(joker_location_keys) do
    if joker_location_list[option] == value then
      return i
    end
  end
  return 1
end

local config = Brainstorm.config
local write_config = Brainstorm.write_config

local function clear_invalid_joker_selection()
  if not (G and G.P_CENTER_POOLS and G.P_CENTER_POOLS.Joker) then
    return false
  end
  if
    config.ar_filters.joker_name ~= ""
    and joker_list[config.ar_filters.joker_name] == nil
  then
    config.ar_filters.joker_name = ""
    config.ar_filters.joker_id = 1
    return true
  end
  return false
end

G.FUNCS.change_target_voucher = function(x)
  config.ar_filters.voucher_id = x.to_key
  config.ar_filters.voucher_name = voucher_list[x.to_val]
  write_config()
end

G.FUNCS.change_target_pack = function(x)
  config.ar_filters.pack_id = x.to_key
  config.ar_filters.pack = pack_list[x.to_val]
  write_config()
end

G.FUNCS.change_target_tag = function(x)
  config.ar_filters.tag_id = x.to_key
  config.ar_filters.tag_name = tag_list[x.to_val]
  write_config()
end

G.FUNCS.change_target_tag2 = function(x)
  config.ar_filters.tag2_id = x.to_key
  config.ar_filters.tag2_name = tag_list[x.to_val]
  write_config()
end

G.FUNCS.change_search_joker = function(x)
  config.ar_filters.joker_id = x.to_key
  config.ar_filters.joker_name = joker_list[x.to_val] or ""
  write_config()
end

local function refresh_brainstorm_tab()
  if not (G and G.OVERLAY_MENU) then
    return
  end
  local tab_button = G.OVERLAY_MENU:get_UIE_by_ID("tab_but_Brainstorm")
  if tab_button then
    G.FUNCS.change_tab(tab_button)
  end
end

G.FUNCS.apply_joker_filter = function()
  local search = config.ar_filters.joker_search
  if type(search) ~= "string" then
    search = tostring(search or "")
  end
  search = search:gsub("^%s+", ""):gsub("%s+$", "")
  local changed = config.ar_filters.joker_search ~= search
  config.ar_filters.joker_search = search
  rebuild_joker_options()
  changed = clear_invalid_joker_selection() or changed
  if changed then
    write_config()
  end
  refresh_brainstorm_tab()
end

G.FUNCS.reset_brainstorm_settings = function()
  Brainstorm.reset_config()
  write_config()
  refresh_brainstorm_tab()
end

G.FUNCS.change_search_joker_location = function(x)
  config.ar_filters.joker_location_id = x.to_key
  config.ar_filters.joker_location = joker_location_list[x.to_val] or "any"
  write_config()
end

G.FUNCS.change_soul_count = function(x)
  config.ar_filters.soul_skip = x.to_val
  write_config()
end

G.FUNCS.change_spf = function(x)
  local spf_key = tostring(x.to_val or "")
  local spf_value = spf_list[spf_key]
  if spf_value then
    config.ar_prefs.spf_id = clamp_index(x.to_key, #spf_keys)
    config.ar_prefs.spf_int = spf_value
  else
    local current_key = spf_keys[clamp_index(
      config.ar_prefs.spf_id or 1,
      #spf_keys
    )] or Brainstorm.DEFAULT_SPF_KEY
    config.ar_prefs.spf_id = option_index_for_value(spf_keys, current_key)
    config.ar_prefs.spf_int = spf_list[current_key]
      or spf_list[Brainstorm.DEFAULT_SPF_KEY]
  end
  write_config()
end

G.FUNCS.change_face_count = function(x)
  config.ar_prefs.face_count = x.to_val
  write_config()
end

G.FUNCS.change_suit_ratio = function(x)
  config.ar_prefs.suit_ratio_id = x.to_key
  config.ar_prefs.suit_ratio_percent = x.to_val
  config.ar_prefs.suit_ratio_decimal = ratio_list[x.to_val]
  write_config()
end

function Brainstorm.build_settings_tab()
  return {
    label = "Brainstorm",
    tab_definition_function = function()
      rebuild_joker_options()
      if clear_invalid_joker_selection() then
        write_config()
      end
      local joker_option =
        clamp_index(config.ar_filters.joker_id or 1, #joker_keys)
      if
        config.ar_filters.joker_name
        and config.ar_filters.joker_name ~= ""
      then
        joker_option =
          option_index_for_value(joker_keys, config.ar_filters.joker_name)
      end
      local joker_location_option = clamp_index(
        config.ar_filters.joker_location_id or 1,
        #joker_location_keys
      )
      if
        config.ar_filters.joker_location
        and config.ar_filters.joker_location ~= ""
      then
        joker_location_option =
          location_index_for_value(config.ar_filters.joker_location)
      end
      local tag_option =
        option_index_for_mapping(tag_keys, tag_list, config.ar_filters.tag_name)
      local tag2_option = option_index_for_mapping(
        tag_keys,
        tag_list,
        config.ar_filters.tag2_name
      )
      local voucher_option = option_index_for_mapping(
        voucher_keys,
        voucher_list,
        config.ar_filters.voucher_name
      )
      local pack_option =
        option_index_for_pack(pack_keys, config.ar_filters.pack)
      return {
        n = G.UIT.ROOT,
        config = {
          align = "cm",
          padding = 0.05,
          colour = G.C.CLEAR,
        },
        nodes = {
          {
            n = G.UIT.C,
            config = {
              align = "cm",
              padding = 0.05,
              r = 0.1,
              colour = darken(G.C.UI.TRANSPARENT_DARK, 0.25),
            },
            nodes = {
              create_option_cycle({
                label = "AR: TAG 1 SEARCH",
                scale = 0.8,
                w = 4,
                options = tag_keys,
                opt_callback = "change_target_tag",
                current_option = tag_option,
              }),
              create_option_cycle({
                label = "AR: TAG 2 SEARCH",
                scale = 0.8,
                w = 4,
                options = tag_keys,
                opt_callback = "change_target_tag2",
                current_option = tag2_option,
              }),
              create_option_cycle({
                label = "AR: VOUCHER SEARCH",
                scale = 0.8,
                w = 4,
                options = voucher_keys,
                opt_callback = "change_target_voucher",
                current_option = voucher_option,
              }),
              create_option_cycle({
                label = "AR: PACK SEARCH",
                scale = 0.8,
                w = 4,
                options = pack_keys,
                opt_callback = "change_target_pack",
                current_option = pack_option,
              }),
              {
                n = G.UIT.R,
                config = { align = "cm", padding = 0.05 },
                nodes = {
                  {
                    n = G.UIT.R,
                    config = { align = "cm" },
                    nodes = {
                      {
                        n = G.UIT.T,
                        config = {
                          text = "AR: JOKER FILTER",
                          scale = 0.4,
                          colour = G.C.UI.TEXT_LIGHT,
                        },
                      },
                    },
                  },
                  {
                    n = G.UIT.R,
                    config = { align = "cm", padding = 0.05 },
                    nodes = {
                      create_text_input({
                        ref_table = config.ar_filters,
                        ref_value = "joker_search",
                        prompt_text = "Filter jokers...",
                        text_scale = 0.3,
                        w = 2.6,
                        h = 0.6,
                        max_length = 24,
                        extended_corpus = true,
                      }),
                      UIBox_button({
                        label = { "Apply" },
                        button = "apply_joker_filter",
                        minw = 0.9,
                        scale = 0.3,
                        col = true,
                        colour = G.C.BLUE,
                      }),
                    },
                  },
                },
              },
              create_option_cycle({
                label = "AR: JOKER SEARCH",
                scale = 0.8,
                w = 4,
                options = joker_keys,
                opt_callback = "change_search_joker",
                current_option = joker_option,
              }),
              create_option_cycle({
                label = "AR: JOKER LOCATION",
                scale = 0.8,
                w = 4,
                options = joker_location_keys,
                opt_callback = "change_search_joker_location",
                current_option = joker_location_option,
              }),
            },
          },
          {
            n = G.UIT.C,
            config = {
              align = "cm",
              padding = 0.05,
              r = 0.1,
              colour = darken(G.C.UI.TRANSPARENT_DARK, 0.25),
            },
            nodes = {
              create_toggle({
                label = "Enable Brainstorm",
                scale = 0.8,
                ref_table = config,
                ref_value = "enable",
                callback = write_config,
              }),
              create_option_cycle({
                label = "AR: Seeds per frame",
                scale = 0.8,
                w = 4,
                options = spf_keys,
                opt_callback = "change_spf",
                current_option = clamp_index(
                  Brainstorm.config.ar_prefs.spf_id or 1,
                  #spf_keys
                ),
              }),
              create_toggle({
                label = "AP: USE CUDA",
                scale = 0.8,
                ref_table = Brainstorm.config.ar_prefs,
                ref_value = "use_cuda",
                callback = write_config,
              }),
              create_toggle({
                label = "AR: INST OBSERVATORY",
                scale = 0.8,
                ref_table = Brainstorm.config.ar_filters,
                ref_value = "inst_observatory",
                callback = write_config,
              }),
              create_toggle({
                label = "AR: INST PERKEO",
                scale = 0.8,
                ref_table = Brainstorm.config.ar_filters,
                ref_value = "inst_perkeo",
                callback = write_config,
              }),
              create_option_cycle({
                label = "AR: N. SOULS",
                scale = 0.8,
                w = 4,
                options = { 0, 1 },
                opt_callback = "change_soul_count",
                current_option = (Brainstorm.config.ar_filters.soul_skip or 0)
                  + 1,
              }),
              create_option_cycle({
                label = "ED: Min. # of Face Cards",
                scale = 0.8,
                w = 4,
                options = (function()
                  local opts = {}
                  for i = 0, 35 do
                    opts[#opts + 1] = i
                  end
                  return opts
                end)(),
                opt_callback = "change_face_count",
                current_option = (Brainstorm.config.ar_prefs.face_count or 0)
                  + 1,
              }),
              create_option_cycle({
                label = "ED: Suit Ratio ",
                scale = 0.8,
                w = 4,
                options = ratio_keys,
                opt_callback = "change_suit_ratio",
                current_option = Brainstorm.config.ar_prefs.suit_ratio_id or 1,
              }),
              UIBox_button({
                label = { "Reset All" },
                button = "reset_brainstorm_settings",
                minw = 3.5,
                scale = 0.45,
                colour = G.C.ORANGE,
              }),
              {
                n = G.UIT.R,
                config = { align = "cm", padding = 0.02 },
                nodes = {
                  {
                    n = G.UIT.T,
                    config = {
                      text = "Original: OceanRamen. Rewrite: KRVH. Immolate: MathIsFun0.",
                      scale = 0.28,
                      colour = G.C.UI.TEXT_LIGHT,
                    },
                  },
                },
              },
            },
          },
        },
      }
    end,
    tab_definition_function_args = "Brainstorm",
  }
end

Brainstorm._ui_hooks = Brainstorm._ui_hooks or {}

local function has_brainstorm_tab(tabs)
  for _, tab in ipairs(tabs) do
    if tab and tab.label == "Brainstorm" then
      return true
    end
  end
  return false
end

if not Brainstorm._ui_hooks.create_tabs then
  Brainstorm._ui_hooks.create_tabs = create_tabs
  function create_tabs(args)
    if
      args
      and args.tab_h == 7.05
      and type(args.tabs) == "table"
      and not has_brainstorm_tab(args.tabs)
    then
      args.tabs[#args.tabs + 1] = Brainstorm.build_settings_tab()
    end
    return Brainstorm._ui_hooks.create_tabs(args)
  end
end
