local repo = assert(arg[1], "repository path is required")

package.preload.lovely = function()
  return { mod_dir = repo }
end
package.preload.nativefs = function()
  return {
    getInfo = function()
      return nil
    end,
    read = function()
      return nil
    end,
    write = function()
      return true
    end,
    createDirectory = function()
      return true
    end,
    getDirectoryItems = function()
      return {}
    end,
  }
end
package.preload.ffi = function()
  return {
    cdef = function() end,
    load = function()
      error("native loading is outside this lifecycle smoke")
    end,
  }
end

local original_updates = 0
local started_run
local deleted_runs = 0
local queued_events = {}
local captured_uibox
local removed_boxes = 0
local dynatext_calls = 0

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
    getSaveDirectory = function()
      return repo
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
  SPEEDFACTOR = 1,
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
lighten = function(value)
  return copy_table(value)
end
darken = function(value)
  return copy_table(value)
end
Particles = function() end
DynaText = function()
  dynatext_calls = dynatext_calls + 1
  error("live status must not construct DynaText")
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
local status = Brainstorm.attention_text({
  scale = 1.4,
  ref_table = Brainstorm,
  ref_value = "ar_status_text",
  major = G.play,
})
assert(#queued_events == 1 and not status.cancelled)
assert(queued_events[1].func())
assert(status.AT and captured_uibox)
local text_config = captured_uibox.definition.nodes[1].config
assert(captured_uibox.definition.nodes[1].n == G.UIT.T)
assert(text_config.ref_table == Brainstorm)
assert(text_config.ref_value == "ar_status_text")
assert(text_config.draw_layer == 1 and text_config.shadow == true)
assert(dynatext_calls == 0)

Brainstorm.ar_text = status
Brainstorm.ar_active = true
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
local cancelled = Brainstorm.attention_text({ major = G.play })
assert(#queued_events == 1)
cancelled.cancelled = true
assert(queued_events[1].func())
assert(cancelled.AT == nil)

print("Lua lifecycle smoke: PASS")
